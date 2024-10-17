use std::{
    cmp::min,
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use clap::{Arg, Command};
use shambler::Vector3 as SV3;
use tes3::esp::{self, Cell, EditorId, Header, Plugin, Static, TES3Object};

mod brush_ni_node;
use brush_ni_node::BrushNiNode;
mod map_data;
use map_data::MapData;
mod mesh;
use mesh::Mesh;
mod game_object;
mod surfaces;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args = Command::new("morrobroom")
        .about("Compile trenchbroom .map files into usable Morrowind mods.")
    .override_usage("morrobroom \"Path/to/Map_Name.map\"")
    .arg_required_else_help(true)
    .args(&[
        Arg::new("MAP_NAME")
            .help("Input map file name.")
            .value_parser(validate_input_map)
            .long("map")
            .required(true),
        Arg::new("MW_DIR")
            .help("Morrowind install directory. Due to trenchbroom behavior you should use manually created symlinks or junctions to achieve vfs-like functionality.")
            .value_parser(check_morrowind_directory)
            .long("mw-dir")
            .required(false),
        Arg::new("PLUGIN_PATH")
            .help("Path to output plugun. Can be a new or existing plugin, using absolute or relative paths.")
            .long("out")
            .value_parser(validate_input_plugin),
        Arg::new("SCALE")
            .help("Overall scale to apply to output meshes. Quake and Morrowind use different scales, as may authors, so for accuracy reasons this argument is required.")
            .long("scale")
            .value_parser(validate_scale),
        Arg::new("MODE")
            .help("Whether to compile in openmw, morrowind.exe, or librequake mode.")
            .long("mode")
            .value_parser(validate_mode),
    ])
    .get_matches();

    let map_name = args.get_one::<String>("MAP_NAME").unwrap();
    let scale_mode = args.get_one::<f32>("SCALE").unwrap_or(&1.0);

    let (workdir, map_dir, plugin_name) = match args.get_one::<String>("PLUGIN_PATH") {
        Some(name) => {
            let (wd, md) = create_workdir(name);
            (wd, md, name.to_string())
        }
        None => {
            let (wd, md) = create_workdir(&map_name);
            let name = format!("{wd}/{md}.esp");
            (wd, md, name)
        }
    };

    let mut plugin = esp::Plugin::from_path(&plugin_name).unwrap_or(esp::Plugin::default());

    // Push the cell record to the plugin
    // It can't be done multiple times :/
    let mut cell = None;
    let mut created_objects = Vec::new();
    let mut processed_base_objects: HashSet<String> = HashSet::new();

    let map_data = MapData::new(map_name);

    let mut used_indices: HashSet<u32> = plugin
        .objects_of_type::<Cell>()
        .flat_map(|cell| {
            cell.references.iter().filter_map(
                |((mast_idx, ref_idx), _reference)| {
                    if *mast_idx == 0 {
                        Some(*ref_idx)
                    } else {
                        None
                    }
                },
            )
        })
        .collect();

    assert!(
        map_data.geomap.entity_brushes.len() > 0,
        "No brushes found in map! You probably used an apostrophe in worldspawn properties."
    );

    for (entity_id, brushes) in map_data.geomap.entity_brushes.iter() {
        let prop_map = map_data.get_entity_properties(entity_id);

        let mut mesh = Mesh::from_map(brushes, &map_data, &scale_mode);

        match prop_map.get(&"_tb_id".to_string()) {
            Some(group_id) => {
                // This object is a group
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
                            }
                            println!("Adding {ref_id} to unique group set. This should actually not be generated as part of the mesh, but rather create a new one for this unique object. Then it should be placed in the ESP file and referred to later.");
                            processed_group_objects.push(ref_id.to_string());
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

        let ref_id = match prop_map.get(&"RefId".to_string()) {
            Some(ref_id) => ref_id[..min(ref_id.len(), 32)].to_string(),
            None => {
                // The only entity that ever has this happen should be worldspawn
                let ref_id = format!("{map_dir}-scene-{entity_id}");
                ref_id[..min(ref_id.len(), 32)].to_string()
            }
        };

        let lowest_available_index: u32 = (1..).find(|&n| !used_indices.contains(&n)).unwrap_or(1);

        if processed_base_objects.contains(&ref_id.to_string()) {
            println!("Placing new instance of {ref_id} as ref {lowest_available_index}");
        } else {
            processed_base_objects.insert(ref_id.to_string());
        }

        let mesh_name = format!("{}/{}.nif", map_dir, ref_id);

        // We create the base record for the objects here.
        match prop_map.get(&"classname".to_string()) {
            Some(classname) => match classname.as_str() {
                "world_Activator" => {
                    mesh.game_object = game_object::activator(&prop_map, &ref_id, &mesh_name);
                }
                "item_Alchemy" => {
                    mesh.game_object = game_object::potion(&prop_map, &ref_id, &mesh_name);
                }
                "item_Apparatus" => {
                    mesh.game_object = game_object::apparatus(&prop_map, &ref_id, &mesh_name);
                }
                "item_Armor" => {
                    mesh.game_object = game_object::armor(&prop_map, &ref_id, &mesh_name);
                }
                "item_Book" => {
                    mesh.game_object = game_object::book(&prop_map, &ref_id, &mesh_name);
                }
                "item_Ingredient" => {
                    mesh.game_object = game_object::ingredient(&prop_map, &ref_id, &mesh_name);
                }
                "item_Light" => {
                    // Keep in mind this is for lights made from brushes. We also need to support point lights, so that they don't necessarily have to be associated with an object.
                    mesh.game_object = game_object::light(&prop_map, &ref_id, &mesh_name);
                }
                "worldspawn" => {
                    let mut local_cell = game_object::cell(&prop_map);
                    if local_cell.name.is_empty() {
                        local_cell.name = map_dir.clone();
                    }

                    processed_base_objects.extend([local_cell.name.clone(), ref_id.clone()]);

                    cell = Some(local_cell);
                    mesh.game_object = TES3Object::Static(Static {
                        id: ref_id.to_owned(),
                        mesh: mesh_name.to_owned(),
                        flags: esp::ObjectFlags::default(),
                    });
                }
                "func_group" => {
                    processed_base_objects.insert(ref_id.clone());
                    mesh.game_object = TES3Object::Static(Static {
                        id: ref_id.to_owned(),
                        mesh: mesh_name.to_owned(),
                        ..Default::default()
                    })
                }
                _ => {
                    println!(
                        "No matching object type found! {classname} requested for {entity_id}"
                    );
                    continue;
                } // Object has a class, but we don't know what it was.
            },
            None => {}
        }

        let mesh_distance: SV3 = Mesh::centroid(&mesh.node_distances) * (*scale_mode as f32);
        mesh.final_distance = mesh_distance;
        mesh.mangle = match get_prop("mangle", &prop_map) {
            mangle if mangle.is_empty() => *get_rotation(&"0 0 0".to_string()),
            mangle => *get_rotation(&mangle),
        };

        // Also use linked groups to determine if the mesh & base def should be ignored
        // Also we should probably just not check this way *only* and
        // also destroy matching objects once the refId has been determined.
        if !created_objects.contains(&mesh.game_object) {
            let mesh_path = format!("{workdir}/Meshes/{mesh_name}");
            println!("Saving base object definition & mesh for {ref_id} to plugin as {mesh_path}");
            mesh.save(&mesh_path);
            created_objects.push(mesh.game_object.clone());
        }

        if let Some(ref mut local_cell) = cell {
            local_cell.references.insert(
                (0 as u32, lowest_available_index),
                esp::Reference {
                    id: ref_id.to_owned(),
                    mast_index: 0 as u32,
                    refr_index: lowest_available_index,
                    translation: [mesh_distance.x, mesh_distance.y, mesh_distance.z],
                    rotation: [-mesh.mangle[0], -mesh.mangle[1], -mesh.mangle[2]],
                    ..Default::default()
                },
            );

            used_indices.insert(lowest_available_index);
        }
    }

    if let Some(cell) = cell {
        created_objects.push(esp::TES3Object::Cell(cell));
    }

    plugin
        .objects
        .retain(|obj| !processed_base_objects.contains(&obj.editor_id().to_string()));
    plugin.objects.extend(created_objects);
    create_header_if_missing(&mut plugin);
    plugin.sort_objects();
    plugin
        .save_path(&plugin_name)
        .expect("Saving plugin failed!");

    println!("Wrote {plugin_name} to disk successfully.");
}

fn get_rotation(str: &String) -> Box<[f32; 3]> {
    let rot: Vec<&str> = str.split_whitespace().collect();
    let mut array = [0.0f32; 3];

    for (index, axis) in rot.iter().enumerate() {
        array[index] = axis.parse::<f32>().unwrap_or_default().to_radians();
    }

    Box::new([array[2], array[0], array[1]])
}

/// Should probably make some specific struct for handling ESP objects
fn create_header_if_missing(plugin: &mut Plugin) {
    match plugin.objects_of_type::<Header>().count() {
        0 => {
            // Later during serialization, we should make sure to include author and header info.
            plugin.objects.push(TES3Object::Header(Header {
                version: 1.3,
                ..Default::default()
            }));
        }
        _ => {
            println!(
                "Plugin was found to already have {} header records",
                plugin.objects_of_type::<Header>().count()
            )
        }
    }
}

fn create_workdir(map_name: &String) -> (String, String) {
    let dir_index = map_name
        .rfind('/')
        .expect("Map should always have an extension, this is probably a directory");

    let ext_index = map_name
        .rfind('.')
        .expect("Map should always have an extension, this is probably a directory");

    let workdir = &map_name[..dir_index];
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

fn validate_map_extension(path: &Path) -> Result<(), String> {
    let ext = get_extension(path);
    if matches!(&*ext, "map") {
        return Ok(());
    }
    Err(format!("\"{}\" is not a map file!.", path.display()))
}

fn validate_input_plugin(arg: &str) -> Result<String, String> {
    if arg != "-" {
        let path = arg.as_ref();
        validate_plugin_extension(path)?;
        println!(
            "Warning! {} does not already exist. Creating a new plugin.",
            arg
        );
    }
    Ok(arg.into())
}

fn validate_plugin_extension(path: &Path) -> Result<(), String> {
    let ext = get_extension(path);
    if matches!(&*ext, "esp" | "esm" | "omwaddon" | "omwgame") {
        return Ok(());
    }
    Err(format!(
        "\"{}\" is not an Elder Scrolls plugin file!.",
        path.display()
    ))
}

fn validate_mode(arg: &str) -> Result<String, String> {
    match arg {
        "vanilla" => Ok(arg.into()),
        "openmw" => Ok(arg.into()),
        "librequake" => Ok(arg.into()),
        "mw" => Ok(arg.into()),
        "lq" => Ok(arg.into()),
        _ => Err(format!("\"{}\" is not a valid mode.", arg)),
    }
}

fn validate_scale(arg: &str) -> Result<f32, String> {
    arg.parse::<f32>()
        .map_err(|e| format!("Invalid scale value '{}': {}", arg, e))
        .and_then(|num| {
            if num == 0.0 {
                Err("Scale value must be greater than 0".to_string())
            } else {
                Ok(num)
            }
        })
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

fn get_prop(prop_name: &str, prop_map: &HashMap<&String, &String>) -> String {
    prop_map
        .get(&prop_name.to_string())
        .unwrap_or(&&String::default())
        .to_string()
}
