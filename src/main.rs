use std::{collections::HashMap, fs, path::Path};

use clap::{Arg, ArgAction, Command};
use shambler::Vector3 as SV3;
use tes3::{
    esp::{self, EditorId, Header, ObjectFlags, Plugin},
    nif::NiTriShapeData,
};

mod brush_ni_node;
use brush_ni_node::BrushNiNode;
mod map_data;
use map_data::MapData;
mod mesh;
use mesh::Mesh;
mod surfaces;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args = Command::new("morrobroom")
        .about("Compile trenchbroom .map files into usable Morrowind mods.")
    .override_usage("morrobroom \"Map_Name.map\" \"Path/To/Morrowind/Data Files/\"")
    .arg_required_else_help(true)
    .args(&[
        Arg::new("MAP_NAME")
            .help("Input map file name.")
            .value_parser(validate_input_map)
            .required(true),
        Arg::new("MW_DIR")
            .help("Morrowind install directory. Due to trenchbroom behavior you should use manually created symlinks or junctions to achieve vfs-like functionality.")
            .value_parser(check_morrowind_directory)
            .required(true),
        Arg::new("PLUGIN_NAME")
            .help("Output plugin name. Can be a new or existing plugin.")
            .value_parser(validate_input_plugin),
    ])
    .get_matches();

    let mw_dir = args.get_one::<String>("MW_DIR").unwrap();
    let map_name = args.get_one::<String>("MAP_NAME").unwrap();
    let (workdir, map_dir) = create_workdir(&map_name);
    // Default plugin name is just the name of the map, but esp instead.
    let map_id = &map_name[..map_name.len() - 4].to_string();

    // println!("Map ID is: {workdir}/{map_dir}.esp");
    let plugin_str = format!("{workdir}/{map_dir}.esp");

    let plugin_name = match args.get_one::<String>("PLUGIN_NAME") {
        Some(name) => name,
        None => &plugin_str,
    };

    let mut plugin = esp::Plugin::from_path(plugin_name).unwrap_or(esp::Plugin::default());

    create_header_if_missing(&mut plugin);

    // Push the cell record to the plugin
    // It can't be done multiple times :/
    let mut cell = match plugin
        .objects_of_type_mut::<esp::Cell>()
        .find(|obj| obj.name == map_dir)
    {
        Some(cell) => {
            cell.references.clear();
            cell.to_owned()
        }
        None => esp::Cell::default(),
    };

    cell.data.flags = esp::CellFlags::IS_INTERIOR;
    cell.name = map_dir.clone();

    cell.atmosphere_data = Some(esp::AtmosphereData {
        ambient_color: [255, 255, 255, 255],
        fog_density: 1. as f32,
        sunlight_color: [255, 255, 255, 255],
        fog_color: [255, 255, 255, 255],
    });

    // println!("MW_Dir is: {mw_dir}");

    // println!("Workdir is: {workdir}");

    let map_data = MapData::new(map_name);
    let mut processed_base_objects: HashMap<String, String> = HashMap::new();

    let mut indices: u32 = 0;

    for (entity_id, brushes) in map_data.geomap.entity_brushes.iter() {
        let prop_map = map_data.get_entity_properties(entity_id);

        let mut mesh = Mesh::from_map(brushes, &map_data);

        match prop_map.get(&"_tb_id".to_string()) {
            Some(group_id) => {
                println!(
                    "This object is a group! Finding all non-group children for group {group_id}"
                );
                let mut ref_instances = 0;
                let mut nodes = Vec::new();
                let mut processed_group_objects: Vec<String> = Vec::new();

                for (entity_id, brushes) in map_data.geomap.entity_brushes.iter() {
                    let prop_map = map_data.get_entity_properties(entity_id);
                    // let group_id;

                    match prop_map.get(&"_tb_id".to_string()) {
                        Some(_) => continue,
                        None => {}
                    }

                    // We also should account for linked groups in the case below!
                    match prop_map.get(&"_tb_group".to_string()) {
                        Some(obj_group) => {
                            if obj_group != group_id {
                                // println!("Found another group! Bailing on creating this mesh and saving it into the cellref.");
                                continue;
                            };
                        }
                        None => {
                            // println!("This object isn't part of a group, don't do anything with it here.");
                            continue;
                        }
                    }

                    match prop_map.get(&"RefId".to_string()) {
                        Some(ref_id) => {
                            ref_instances += 1;
                            if processed_group_objects.contains(ref_id) {
                                println!("We don't have full refId support yet, but this object {ref_id} has appeared in this group {ref_instances} times"); // In theory by this point, we should have a mesh for this object already.
                                                                                                                                                             // Alternatively, we have to generate it here, which is probably going to be likely.
                                continue; // If it does exist, though, we need to simply derive its placement
                            } else {
                                println!("Adding {ref_id} to unique group set. This should actually not be generated as part of the mesh, but rather create a new one for this unique object. Then it should be placed in the ESP file and referred to later.");
                                processed_group_objects.push(ref_id.to_string());
                            }
                        }
                        None => {} // object has no refid, and it's not a group, but it is a member of a group. This maybe shouldn't happen
                    }

                    nodes.extend(BrushNiNode::from_brushes(brushes, &map_data));
                }

                for node in nodes {
                    mesh.attach_node(node);
                }
            }
            None => {}
        }

        let mesh_distance = find_closest_vertex(&mesh.node_distances);

        if let Some(base_node) = mesh.stream.get_mut(mesh.base_index) {
            base_node.translation = [-mesh_distance.x, -mesh_distance.y, -mesh_distance.z].into()
        }

        // mesh.stream
        //     .objects_of_type_mut::<NiTriShapeData>()
        //     .flat_map(|shape| shape.vertices.iter_mut())
        //     .for_each(|vertex| {
        //         vertex.x -= mesh_distance.x;
        //         vertex.y -= mesh_distance.y;
        //         vertex.z -= mesh_distance.z;
        //     });

        let ref_id = match prop_map.get(&"RefId".to_string()) {
            Some(ref_id) => {
                if processed_base_objects.contains_key(&ref_id.to_string()) {
                    // Not sure what to do here.
                    // In this case, the object has already had a mesh generated.
                    // But, we need to collect its vertices to know what position to provide it.
                    // Maybe we should just have a specific function or behavior that only gets the vertices here.
                    // Starting to get ugly.
                    // continue;
                    println!("Already processed {ref_id} mesh, it should only have a cellRef and not a new mesh.");
                } else {
                    println!("Adding {ref_id} to unique set");
                    // processed_base_objects.push(ref_id.to_string());
                }
                ref_id.to_string()
            }
            None => {
                format!("scene_{entity_id}")
                // println!("This object has no refid, and isn't part of a group. It may be the worldspawn?");
            }
        };

        let mesh_name = format!("{workdir}/Meshes/{map_dir}/{ref_id}.nif");

        println!("Saving mesh as {mesh_name}");

        // We create the base record for the objects here.
        match prop_map.get(&"classname".to_string()) {
            Some(classname) => match classname.as_str() {
                "BOOK" => {
                    mesh.game_object = esp::TES3Object::Book(tes3::esp::Book {
                        id: ref_id.clone(),
                        mesh: format!("{map_dir}/{ref_id}.nif"),
                        ..Default::default()
                    })
                }
                _ => {
                    mesh.game_object = esp::TES3Object::Static(tes3::esp::Static {
                        id: ref_id.clone(),
                        mesh: format!("{map_dir}/{ref_id}.nif"),
                        ..Default::default()
                    })
                }
            },
            None => {
                mesh.game_object = esp::TES3Object::Book(tes3::esp::Book {
                    id: ref_id.clone(),
                    mesh: format!("{map_dir}/{ref_id}.nif"),
                    ..Default::default()
                })
            } // object has no refid, and it's not a group, but it is a member of a group. This maybe shouldn't happen
        }

        // Also use linked groups to determine if the mesh & base def should be ignored
        if !plugin.objects.contains(&mesh.game_object) {
            println!("Saving mesh and base object definition for {ref_id} to plugin");
            mesh.save(&mesh_name);
            plugin.objects.push(mesh.game_object);
        }

        let new_cellref = esp::Reference {
            id: ref_id.clone(),
            mast_index: 0 as u32,
            refr_index: indices,
            translation: [mesh_distance.x, mesh_distance.y, mesh_distance.z],
            ..Default::default()
        };

        cell.references.insert((0 as u32, indices), new_cellref);

        processed_base_objects.insert(ref_id, mesh_name);

        indices += 1;
    }

    plugin.objects.retain(|obj| obj.editor_id() != cell.name);
    plugin.objects.push(esp::TES3Object::Cell(cell));
    plugin
        .save_path(plugin_name)
        .expect("Saving plugin failed!");

    println!("Wrote {plugin_name} to disk successfully.");
}

fn find_closest_vertex(verts: &Vec<SV3>) -> SV3 {
    if verts.len() == 0 {
        println!("WARNING: Empty vertex set had a position requested. This object has probably been clipped out of visibility space. Returning 0, 0, 0 for this object's position.");
        return SV3::default();
    }

    // Initialize the closest vertex with the first vertex in vis_data
    let mut closest_vertex = verts[0];
    // Initialize the maximum distance squared with the squared distance of the first vertex
    let mut max_distance_squared =
        verts[0].x * verts[0].x + verts[0].y * verts[0].y + verts[0].z * verts[0].z;

    // Iterate over the remaining provided vertices starting from the second vertex
    for vertex in verts.iter().skip(1) {
        // Calculate the squared distance from the origin for the current vertex
        let distance_squared = vertex.x * vertex.x + vertex.y * vertex.y + vertex.z * vertex.z;

        // If the calculated distance is lower than the current maximum, update the closest vertex
        if distance_squared < max_distance_squared {
            closest_vertex = *vertex;
            max_distance_squared = distance_squared;
        }
    }

    // Return the closest vertex
    closest_vertex
}

/// Should probably make some specific struct for handling ESP objects
fn create_header_if_missing(plugin: &mut Plugin) {
    match plugin.objects_of_type::<esp::Header>().count() {
        0 => {
            let mut header = esp::Header {
                version: 1.3,
                ..Default::default()
            };
            // Later during serialization, we should make sure to include author and header info.
            plugin.objects.push(esp::TES3Object::Header(header));
        }
        _ => {}
    }
}

fn validate_input_map(arg: &str) -> Result<String, String> {
    if arg != "-" {
        let path = arg.as_ref();
        validate_map_extension(path)?;
        if !path.exists() {
            return Err(format!("\"{}\" (file does not exist).", path.display()));
        }
    }
    Ok(arg.into())
}

fn validate_input_plugin(arg: &str) -> Result<String, String> {
    if arg != "-" {
        let path = arg.as_ref();
        validate_plugin_extension(path)?;
        if !path.exists() {
            return Err(format!("\"{}\" (file does not exist).", path.display()));
        }
    }
    Ok(arg.into())
}

fn create_workdir(map_name: &String) -> (String, String) {
    let dir_index = map_name
        .rfind('/')
        .expect("Map should always have an extension, this is probably a directory");

    let ext_index = map_name
        .rfind('.')
        .expect("Map should always have an extension, this is probably a directory");

    let workdir = &map_name[..dir_index - 3];
    let map_dir = &map_name[dir_index + 1..ext_index];

    if !fs::metadata(format!("{workdir}")).is_ok() {
        fs::create_dir(format!("{workdir}"))
            .expect("Root workdir folder creation failed! This is very bad!")
    }

    if !fs::metadata(format!("{workdir}/Meshes/")).is_ok() {
        fs::create_dir(format!("{workdir}/Meshes/"))
            .expect("Workdir meshes folder creation failed! This is very bad!")
    }

    if !fs::metadata(format!("{workdir}/Meshes/{map_dir}")).is_ok() {
        fs::create_dir(format!("{workdir}/Meshes/{map_dir}"))
            .expect("Workdir map folder creation failed! This is very bad!")
    }

    (workdir.to_string(), map_dir.to_string())
}

fn validate_map_extension(path: &Path) -> Result<(), String> {
    let ext = get_extension(path);
    if matches!(&*ext, "map") {
        return Ok(());
    }
    Err(format!("\"{}\" is not a map file!.", path.display()))
}

fn validate_plugin_extension(path: &Path) -> Result<(), String> {
    let ext = get_extension(path);
    if matches!(&*ext, "esp" | "esm" | "omwaddon" | "omwgame") {
        return Ok(());
    }
    Err(format!("\"{}\" is not a map file!.", path.display()))
}

fn get_extension(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}

fn check_morrowind_directory(dir_path: &str) -> Result<String, String> {
    let path = std::path::Path::new(dir_path);

    if !path.exists() {
        return Err(format!("Directory '{}' does not exist.", dir_path));
    }

    if !path.is_dir() {
        return Err(format!("'{}' is not a directory.", dir_path));
    }

    let esm_path = path.join("Morrowind.esm");
    if !esm_path.exists() {
        return Err(format!(
            "'{}' does not appear to be a valid Morrowind directory as it does not contain Morrowind.esm.",
            dir_path
        ));
    }

    Ok(dir_path.to_string())
}
