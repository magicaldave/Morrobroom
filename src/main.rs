use imagesize::size;
use openmw_cfg::{find_file, get_config, Ini};
use shalrath::repr::*;
use shambler::{
    brush::BrushId,
    entity::EntityId,
    face::{FaceNormals, FaceTriangleIndices, FaceUvs, FaceVertices},
    texture::{TextureId, TextureSizes},
    GeoMap, Textures, TexturesTag, Vector2 as SV2, Vector3 as SV3,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
    path::PathBuf,
};
use tes3::{
    esp::*,
    nif::{
        self, NiLink, NiNode, NiStream, NiTriShape, NiTriShapeData, RootCollisionNode,
        TextureSource,
    },
};

enum NiBroomSurface {
    NoClip = 1,
    Phong = 2,
    Invert = 4,
}

struct MapData {
    geomap: GeoMap,
    face_vertices: FaceVertices,
    face_tri_indices: FaceTriangleIndices,
    inverted_face_tri_indices: FaceTriangleIndices,
    flat_normals: FaceNormals,
    smooth_normals: FaceNormals,
    texture_names: HashSet<String>,
    texture_paths: HashSet<String>,
    face_uvs: FaceUvs, // texture_sizes: BTreeMap<String, (u32, u32)>,
}

impl MapData {
    pub fn new(map_name: &String) -> Self {
        let map = fs::read_to_string(map_name)
            .expect("Reading file failed. Bad news! Does it exist?")
            .parse::<Map>()
            .expect("Map parsing failed!");

        let geomap = GeoMap::new(map);

        let face_planes = shambler::face::face_planes(&geomap.face_planes);
        let brush_hulls = shambler::brush::brush_hulls(&geomap.brush_faces, &face_planes);
        let (face_vertices, face_vertex_planes) =
            shambler::face::face_vertices(&geomap.brush_faces, &face_planes, &brush_hulls);
        let face_centers = shambler::face::face_centers(&face_vertices);
        let face_indices = shambler::face::face_indices(
            &geomap.face_planes,
            &face_planes,
            &face_vertices,
            &face_centers,
            shambler::face::FaceWinding::Clockwise,
        );

        // If a brush is marked as "inside-out", we use these indices instead
        let inverted_face_indices = shambler::face::face_indices(
            &geomap.face_planes,
            &face_planes,
            &face_vertices,
            &face_centers,
            shambler::face::FaceWinding::CounterClockwise,
        );

        let face_tri_indices = shambler::face::face_triangle_indices(&face_indices);
        let inverted_face_tri_indices =
            shambler::face::face_triangle_indices(&inverted_face_indices);
        let flat_normals = shambler::face::normals_flat(&face_vertices, &face_planes);
        let smooth_normals =
            shambler::face::normals_phong_averaged(&face_vertex_planes, &face_planes);

        let texture_names = MapData::collect_textures(&geomap.textures);
        let texture_paths = MapData::find_textures_in_vfs(&texture_names);

        let texture_sizes: BTreeMap<&str, (u32, u32)> = texture_paths
            .iter()
            .map(|texture_name| {
                let texture_size = size(texture_name.clone()).expect(&format!(
                    "Image Processing failed! Is there an issue with the path? {}",
                    texture_name
                ));
                (
                    texture_name.as_str(),
                    (texture_size.width as u32, texture_size.height as u32),
                )
            })
            .collect();

        let mut modified_textures: BTreeMap<TextureId, String> = BTreeMap::new();

        for (texture_id, texture_name) in geomap.textures.iter() {
            if texture_name == "__TB_empty" || texture_name == "skip" || texture_name == "clip" {
                continue;
            }

            for texture_path in &texture_paths {
                if texture_path.contains(texture_name) {
                    modified_textures.insert(*texture_id, texture_path.to_string());
                }
            }
            // println!("{texture_name:?}");
        }

        let mut textures_with_paths: Textures = Textures::default();
        textures_with_paths.data = modified_textures;

        let face_uvs = shambler::face::new(
            &geomap.faces,
            &geomap.textures,
            &geomap.face_textures,
            &face_vertices,
            &face_planes,
            &geomap.face_offsets,
            &geomap.face_angles,
            &geomap.face_scales,
            &shambler::texture::texture_sizes(&textures_with_paths, texture_sizes),
        );

        // println!("{:?}", texture_paths);

        MapData {
            geomap,
            face_vertices,
            face_tri_indices,
            inverted_face_tri_indices,
            flat_normals,
            smooth_normals,
            texture_names,
            texture_paths,
            face_uvs, // texture_sizes: BTreeMap::new(),
        }
    }

    fn collect_textures(textures: &Textures) -> HashSet<String> {
        textures
            .iter()
            .map(|(_, texture_name)| texture_name.to_string())
            .collect()
    }

    fn find_vfs_texture(name: &str, config: &Ini) -> Option<String> {
        let extensions = ["dds", "tga"];

        if name == "__TB_empty" || name == "skip" || name == "clip" {
            return None;
        }

        Some(
            extensions
                .iter()
                .flat_map(|extension| {
                    find_file(config, format!("Textures/{}.{}", name, extension).as_str())
                })
                .next()
                .expect("Can't find texture! Somehow this map is using a texture which isn't in your openmw vfs.")
                .to_string_lossy()
                .to_string(),
        )
    }

    fn find_textures_in_vfs(textures: &HashSet<String>) -> HashSet<String> {
        let config = get_config().expect("Openmw.cfg not detected! Please ensure you have a valid openmw configuration file in the canonical system directory.");
        let texture_paths: HashSet<String> = textures
            .iter()
            .filter_map(|texture_name| MapData::find_vfs_texture(&texture_name, &config))
            .collect();

        texture_paths
    }

    fn get_entity_properties(&self, entity_id: &EntityId) -> HashMap<&String, &String> {
        let entity_properties = self.geomap.entity_properties.get(&entity_id);

        // We *need* to handle groups here
        // Group names are powers of 2 and have different keys in the group definition and separate entities which reference it
        if let None = entity_properties {
            panic!("brush entity {} has no properties!", entity_id);
        }

        entity_properties
            .unwrap()
            .iter()
            .fold(HashMap::new(), |mut acc, prop| {
                acc.insert(&prop.key, &prop.value);
                acc
            })
    }
}

struct Mesh {
    stream: NiStream,
    root_index: NiLink<NiNode>,
    collision_index: NiLink<RootCollisionNode>,
    use_collision_root: bool,
}

impl Mesh {
    fn new() -> Self {
        let mut stream = NiStream::default();

        let root_node = NiNode::default();
        let root_index = stream.insert(root_node);
        // let collision_root = RootCollisionNode::default();
        // let collision_index = 0;

        stream.roots = vec![root_index.cast()];

        Mesh {
            stream,
            root_index,
            // collision_root,
            collision_index: NiLink::<RootCollisionNode>::default(),
            use_collision_root: false,
        }
    }

    fn attach_collision(&mut self) {
        self.use_collision_root = true;
        self.collision_index = self.stream.insert(RootCollisionNode::default());

        if let Some(root) = self.stream.get_mut(self.root_index) {
            root.children.push(self.collision_index.cast());
        };
    }

    fn from_map(brushes: &Vec<BrushId>, map_data: &MapData) -> Mesh {
        let mut mesh = Mesh::new();

        // Every brush should be a unique TriShape
        // However, faces which use multiple textures must be split further
        // Perhaps we store the texture string and the vertex data in a hashmap,
        // keyed against the texture string
        for brush_id in brushes {
            let node = BrushNiNode::from_brush(brush_id, map_data);

            if node.col_verts.len() != node.vis_verts.len() && !mesh.use_collision_root {
                println!("Enabling root collision from on brush {brush_id}");
                mesh.attach_collision();
            }

            mesh.attach_node(node, &map_data.texture_paths);
        }
        mesh
    }

    fn save(&mut self, name: String) {
        let _ = self.stream.save_path(name);
    }

    fn attach_node(&mut self, node: BrushNiNode, texture_paths: &HashSet<String>) {
        if node.vis_verts.len() > 0 {
            let vis_index = self.stream.insert(node.vis_shape);

            // println!("{0}", node.texture);

            self.assign_texture(vis_index, node.texture);

            let vis_data_index = self.stream.insert(node.vis_data);

            if let Some(shape) = self.stream.get_mut(vis_index) {
                shape.geometry_data = vis_data_index.cast();
            };

            if let Some(root) = self.stream.get_mut(self.root_index) {
                root.children.push(vis_index.cast());
            };
        }

        if self.use_collision_root == true {
            // We need to figure out how to omit the collision node, should it be identical to vis data

            if node.col_verts.len() > 0 {
                let col_index = self.stream.insert(node.col_shape);

                let col_data_index = self.stream.insert(node.col_data);

                if let Some(collision) = self.stream.get_mut(col_index) {
                    collision.geometry_data = col_data_index.cast();
                };

                if let Some(collision_root) = self.stream.get_mut(self.collision_index) {
                    collision_root.children.push(col_index.cast());
                };
            }
        }
    }

    fn assign_texture(&mut self, object: nif::NiLink<nif::NiTriShape>, file_path: String) {
        let config =
            get_config().expect("Openmw.cfg not located! Be sure you have a valid openmw setup.");
        // Create and insert a NiTexturingProperty and NiSourceTexture.
        let tex_prop_link = self.stream.insert(nif::NiTexturingProperty::default());
        let texture_link = self.stream.insert(nif::NiSourceTexture::default());

        let mut extension = String::default();

        for extension_candidate in ["dds", "tga"] {
            let candidate_path = format!("Textures/{file_path}.{extension_candidate}");
            if let Ok(rel_path) = find_file(&config, candidate_path.as_str()) {
                extension = extension_candidate.to_string();
                println!("Texture found! {extension_candidate}");
                continue;
            }
        }

        println!("{file_path}");

        // Update the base map texture.
        let tex_prop = self.stream.get_mut(tex_prop_link).unwrap();
        tex_prop.texture_maps.resize(7, None); // not sure why
        let mut base_map = nif::Map::default();
        base_map.texture = texture_link.cast();
        tex_prop.texture_maps[0] = Some(nif::TextureMap::Map(base_map));

        // Update the texture source path.
        let texture = self.stream.get_mut(texture_link).unwrap();
        texture.source = nif::TextureSource::External(format!("{file_path}.{extension}").into());
        println!("{0:?}", texture.source);

        // Assign the tex prop to the target object
        let object = self.stream.get_mut(object).unwrap();
        object.properties.push(tex_prop_link.cast());
    }
}

struct BrushNiNode {
    vis_shape: NiTriShape,
    vis_data: NiTriShapeData,
    vis_verts: Vec<SV3>,
    vis_tris: Vec<Vec<usize>>,
    texture: String,
    normals: Vec<SV3>,
    col_shape: NiTriShape,
    col_data: NiTriShapeData,
    col_verts: Vec<SV3>,
    col_tris: Vec<Vec<usize>>,
    uv_sets: Vec<SV2>,
}

impl BrushNiNode {
    fn from_brush(brush_id: &BrushId, map_data: &MapData) -> Self {
        let mut node = BrushNiNode::default();
        let faces = map_data.geomap.brush_faces.get(brush_id).unwrap();

        for face_id in faces.iter() {
            let texture_id = map_data.geomap.face_textures.get(face_id).unwrap();
            let texture_name = map_data.geomap.textures.get(texture_id).unwrap();

            if texture_name == "skip" {
                continue;
            };
            // Before moving onto texture generation,

            let s_flags = match &map_data.geomap.face_extensions.get(face_id) {
                Some(Extension::Quake2 {
                    content_flags: _,
                    surface_flags,
                    value: _,
                }) => *surface_flags,
                _ => 0,
            };

            let vertices = &map_data.face_vertices.get(&face_id).unwrap();

            // Later, we'll need to do something extra on the brush to define whether it can be inverted
            let indices = match map_data.face_tri_indices.get(&face_id) {
                Some(indices) => indices,
                None => {
                    continue;
                }
            };

            let uv_sets = &map_data.face_uvs.get(&face_id).unwrap();

            if texture_name != "clip" {
                node.normals
                    .extend(&*map_data.flat_normals.get(&face_id).unwrap());
                node.uv_sets.extend(*uv_sets);
                node.vis_verts.extend(*vertices);
                node.vis_tris.push((*indices).to_vec());
                node.texture = texture_name.to_string();
            }

            // There is minor edge case in this approach where if all faces of an object do not have collision then an empty collision root is created
            // This is exactly what we want, but, I worry it will have stupid consequences later
            if s_flags & NiBroomSurface::NoClip as u32 == 0 {
                // println!("Face {face_id} on brush {brush_id} has collision!");
                node.col_verts.extend(*vertices);
                node.col_tris.push((*indices).to_vec());
            }
        }

        node.collect();
        node
    }

    /// Don't forget to deal with collision and attachment!!!!
    fn from_brushes(brushes: &[BrushId], map_data: &MapData) -> Vec<BrushNiNode> {
        brushes
            .iter()
            .map(|brush_id| BrushNiNode::from_brush(brush_id, map_data))
            .collect()
    }

    fn to_nif_format(shape_data: &mut NiTriShapeData, verts: &Vec<SV3>, tris: &Vec<Vec<usize>>) {
        if verts.len() == 0 {
            return;
        };

        let mut verts_used = 0;
        let mut fixed_tris: Vec<[u16; 3]> = Vec::new();

        for face_tris in tris.iter() {
            fixed_tris.extend(face_tris.chunks_exact(3).map(|chunk| {
                [
                    (chunk[0] + verts_used) as u16,
                    (chunk[1] + verts_used) as u16,
                    (chunk[2] + verts_used) as u16,
                ]
            }));

            verts_used += face_tris.into_iter().collect::<HashSet<_>>().len();
        }

        shape_data.triangles = fixed_tris;

        for vertex in verts {
            shape_data
                .vertices
                .push([vertex[0] as f32, vertex[1] as f32, vertex[2] as f32].into());
        }
    }

    fn collect(&mut self) {
        Self::to_nif_format(&mut self.vis_data, &self.vis_verts, &self.vis_tris);
        Self::to_nif_format(&mut self.col_data, &self.col_verts, &self.col_tris);

        for normal in &self.normals {
            self.vis_data
                .normals
                .push([normal[0] as f32, normal[1] as f32, normal[2] as f32].into());
        }

        for uv in &self.uv_sets {
            self.vis_data.uv_sets.push((uv[0], uv[1]).into());
        }
    }
}

impl Default for BrushNiNode {
    fn default() -> BrushNiNode {
        BrushNiNode {
            vis_shape: NiTriShape::default(),
            vis_data: NiTriShapeData::default(),
            vis_verts: Vec::new(),
            vis_tris: Vec::new(),
            texture: String::new(),
            normals: Vec::new(),
            col_shape: NiTriShape::default(),
            col_data: NiTriShapeData::default(),
            col_verts: Vec::new(),
            col_tris: Vec::new(),
            uv_sets: Vec::new(),
        }
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let map_name;

    match args.len() {
        2 => map_name = &args[1],
        _ => panic!(
            "No map to parse! Please provide the path to the desired .map file as an argument."
        ),
    };

    let map_data = MapData::new(map_name);
    let mut processed_base_objects: Vec<String> = Vec::new();

    for (entity_id, brushes) in map_data.geomap.entity_brushes.iter() {
        // So what happens, right?
        // Well, if we identify that this specific entity is a group, we need to gather all its children.
        // If that object is a group, skip it. If it's not a group,
        // Add the NiNodes to the mesh

        let prop_map = map_data.get_entity_properties(entity_id);

        // for (key, value) in &prop_map {
        //     println!("{}, {}", key, value)
        // }

        let mut mesh = Mesh::from_map(brushes, &map_data);

        match prop_map.get(&"RefId".to_string()) {
            Some(ref_id) => {
                if processed_base_objects.contains(ref_id) {
                    continue;
                } else {
                    println!("Adding {ref_id} to unique set");
                    processed_base_objects.push(ref_id.to_string());
                }
            }
            None => {
                println!("This object has no refid, and isn't part of a group. It may be the worldspawn?");
            }
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
                                println!("Found another group! Bailing on creating this mesh and saving it into the cellref.");
                                continue;
                            };
                        }
                        None => {
                            println!("This object isn't part of a group, don't do anything with it here.");
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
                        None => {
                            println!("This object has no refid, and it's not a group, but it is a member of a group. This maybe shouldn't happen.");
                        }
                    }

                    // println!("Object, not group!!");
                    nodes.extend(BrushNiNode::from_brushes(brushes, &map_data));
                }
                println!("Total Child Nodes: {:?}", nodes.len());

                for node in nodes {
                    if node.col_verts.len() != node.vis_verts.len() && !mesh.use_collision_root {
                        // println!("Enabling root collision from on brush {brush_id}");
                        mesh.attach_collision();
                    }

                    mesh.attach_node(node, &map_data.texture_paths);
                }
            }
            None => {}
        }

        // Every entity is its own mesh
        mesh.save(format!("test_{entity_id}.nif"));
    }
}
