use std::{collections::HashMap, env, fs};

use shambler::Vector3 as SV3;
use tes3::{esp, nif::NiTriShapeData};

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
    let args: Vec<_> = env::args().collect();
    let map_name;

    match args.len() {
        2 => map_name = &args[1],
        _ => panic!(
            "No map to parse! Please provide the path to the desired .map file as an argument."
        ),
    };

    let workdir = create_workdir(&map_name);

    let map_data = MapData::new(map_name);
    let mut processed_base_objects: HashMap<String, String> = HashMap::new();

    for (entity_id, brushes) in map_data.geomap.entity_brushes.iter() {
        let prop_map = map_data.get_entity_properties(entity_id);

        let mut mesh = Mesh::from_map(brushes, &map_data);

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

        match prop_map.get(&"classname".to_string()) {
            Some(classname) => match classname.as_str() {
                "BOOK" => {
                    mesh.game_object = esp::TES3Object::Book(tes3::esp::Book {
                        id: ref_id.clone(),
                        ..Default::default()
                    })
                }
                _ => {}
            },
            None => {} // object has no refid, and it's not a group, but it is a member of a group. This maybe shouldn't happen
        }

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

        mesh.stream
            .objects_of_type_mut::<NiTriShapeData>()
            .flat_map(|shape| shape.vertices.iter_mut())
            .for_each(|vertex| {
                vertex.x -= mesh_distance.x;
                vertex.y -= mesh_distance.y;
                vertex.z -= mesh_distance.z;
            });

        let mesh_name = format!("Meshes/{workdir}/test_{entity_id}.nif");

        // Every entity is its own mesh
        mesh.save(&mesh_name);

        processed_base_objects.insert(ref_id, mesh_name);
    }
}

fn create_workdir(map_name: &String) -> String {
    let ext_index = map_name
        .rfind('.')
        .expect("Map should always have an extension, this is probably a directory");

    let workdir = &map_name[..ext_index];

    if !fs::metadata("Meshes").is_ok() {
        fs::create_dir("Meshes").expect("Folder creation failed! This is very bad!")
    }

    if !fs::metadata(format!("Meshes/{workdir}")).is_ok() {
        fs::create_dir(format!("Meshes/{workdir}"))
            .expect("Folder creation failed! This is very bad!")
    }

    workdir.to_string()
}

fn find_closest_vertex(verts: &Vec<SV3>) -> SV3 {
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
