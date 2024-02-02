use shalrath::repr::*;
use shambler::{
    brush::BrushId,
    face::{FaceNormals, FaceTriangleIndices, FaceVertices},
    GeoMap, Vector3 as SV3,
};
use std::{collections::HashSet, env, fs};
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

        MapData {
            geomap,
            face_vertices,
            face_tri_indices,
            inverted_face_tri_indices,
            flat_normals,
            smooth_normals,
        }
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
            // Every brush gets one, EXCEPT for brushes with multiple textures
            let mut node = BrushNiNode::default();
            let faces = map_data.geomap.brush_faces.get(brush_id).unwrap();

            for face_id in faces.iter() {
                let texture_id = map_data.geomap.face_textures.get(face_id).unwrap();
                let texture_name = map_data.geomap.textures.get(texture_id).unwrap();

                if texture_name == "skip" {
                    continue;
                };

                let mut s_flags: u32 = 0;

                // Before moving onto texture generation,

                let s_flags = match &map_data.geomap.face_extensions.get(face_id) {
                    Some(Extension::Quake2 {
                        content_flags: _,
                        surface_flags,
                        value: _,
                    }) => *surface_flags,
                    _ => 0,
                };

                // println!("EXTENSIONS: {s_flags}");

                let vertices = &map_data.face_vertices.get(&face_id).unwrap();

                // Later, we'll need to do something extra on the brush to define whether it can be inverted
                let indices = match map_data.face_tri_indices.get(&face_id) {
                    Some(indices) => indices,
                    None => {
                        continue;
                    }
                };

                if texture_name != "clip" {
                    node.normals
                        .extend(&*map_data.flat_normals.get(&face_id).unwrap());
                    node.vis_verts.extend(*vertices);
                    node.vis_tris.push((*indices).to_vec());
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

            if node.col_verts.len() != node.vis_verts.len() && !mesh.use_collision_root {
                // println!("Enabling root collision from on brush {brush_id}");
                mesh.attach_collision();
            }

            mesh.attach_node(node);
        }
        mesh
    }

    fn save(&mut self, name: String) {
        let _ = self.stream.save_path(name);
    }

    fn attach_node(&mut self, node: BrushNiNode) {
        if node.vis_verts.len() > 0 {
            let vis_index = self.stream.insert(node.vis_shape);

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
}

struct BrushNiNode {
    vis_shape: NiTriShape,
    vis_data: NiTriShapeData,
    vis_verts: Vec<SV3>,
    vis_tris: Vec<Vec<usize>>,
    normals: Vec<SV3>,
    col_shape: NiTriShape,
    col_data: NiTriShapeData,
    col_verts: Vec<SV3>,
    col_tris: Vec<Vec<usize>>,
}

impl BrushNiNode {
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
    }
}

impl Default for BrushNiNode {
    fn default() -> BrushNiNode {
        BrushNiNode {
            vis_shape: NiTriShape::default(),
            vis_data: NiTriShapeData::default(),
            vis_verts: Vec::new(),
            vis_tris: Vec::new(),
            normals: Vec::new(),
            col_shape: NiTriShape::default(),
            col_data: NiTriShapeData::default(),
            col_verts: Vec::new(),
            col_tris: Vec::new(),
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

    for (entity_id, brushes) in map_data.geomap.entity_brushes.iter() {
        let entity_properties = map_data.geomap.entity_properties.get(&entity_id);

        // We *need* to handle groups here
        // Group names are powers of 2 and have different keys in the group definition and separate entities which reference it
        if let None = entity_properties {
            panic!("brush entity {} has no properties!", entity_id);
        }

        // So what happens, right?
        // Well, if we identify that this specific entity is a group, we need to gather all its children.
        // If that object is a group, skip it. If it's not a group,
        // Add the NiNodes to the mesh

        let props = entity_properties.unwrap().iter();

        for prop in props {
            // match (prop.key, prop.value) {
            //     (("_phong".to_string()), "1".to_string()) => {

            //     }
            // }
            println!("{}, {}", prop.key, prop.value)
        }

        let mut mesh = Mesh::from_map(brushes, &map_data);

        // Every entity is its own mesh
        mesh.save(format!("test_{entity_id}.nif"));
    }
}

fn assign_texture(
    stream: &mut nif::NiStream,
    object: nif::NiLink<nif::NiTriShape>,
    file_path: &str,
) {
    // Create and insert a NiTexturingProperty and NiSourceTexture.
    let tex_prop_link = stream.insert(nif::NiTexturingProperty::default());
    let texture_link = stream.insert(nif::NiSourceTexture::default());

    // Update the base map texture.
    let tex_prop = stream.get_mut(tex_prop_link).unwrap();
    tex_prop.texture_maps.resize(7, None); // not sure why
    let mut base_map = nif::Map::default();
    base_map.texture = texture_link.cast();
    tex_prop.texture_maps[0] = Some(nif::TextureMap::Map(base_map));

    // Update the texture source path.
    let texture = stream.get_mut(texture_link).unwrap();
    texture.source = nif::TextureSource::External(file_path.into());

    // Assign the tex prop to the target object
    let object = stream.get_mut(object).unwrap();
    object.properties.push(tex_prop_link.cast());
}
