use std::collections::HashSet;

use shalrath;
use shambler::{brush::BrushId, face::FaceId, Vector2 as SV2, Vector3 as SV3};
use tes3::nif::{NiTriShape, NiTriShapeData};

use crate::{map_data::MapData, surfaces, Mesh};

pub struct BrushNiNode {
    pub vis_shape: NiTriShape,
    pub vis_data: NiTriShapeData,
    pub vis_verts: Vec<SV3>,
    pub use_emissive: bool,
    pub texture: String,
    pub col_shape: NiTriShape,
    pub col_data: NiTriShapeData,
    pub col_verts: Vec<SV3>,
    pub distance_from_origin: SV3,
    // Textures and triangles are only used internally
    normals: Vec<SV3>,
    uv_sets: Vec<SV2>,
    vis_tris: Vec<Vec<usize>>,
    col_tris: Vec<Vec<usize>>,
}

impl BrushNiNode {
    pub fn from_brushes(brushes: &[BrushId], map_data: &MapData) -> Vec<BrushNiNode> {
        brushes
            .iter()
            .flat_map(|brush_id| BrushNiNode::from_brush(brush_id, map_data))
            .collect()
    }

    /// The name of this function might be a bit confusing, as it returns a set of nodes
    /// But one brush may have multiple textures, whereas one TriShape should only
    /// ever have one texture. So even though we are requesting information for one brush,
    /// Any one brush might be an arbitrary number of TriShapes due to texture splitting.
    pub fn from_brush(brush_id: &BrushId, map_data: &MapData) -> Vec<BrushNiNode> {
        let mut face_nodes = Vec::new();

        let faces_with_textures = Self::collect_faces_with_textures(brush_id, map_data);

        for face_set in faces_with_textures {
            face_nodes.push(Self::node_from_faces(&face_set, &map_data));
        }

        for node in &mut face_nodes {
            node.collect()
        }

        face_nodes
    }

    fn node_from_faces(faces: &Vec<FaceId>, map_data: &MapData) -> BrushNiNode {
        let mut node = BrushNiNode::default();

        for face_id in faces.iter() {
            let texture_id = map_data.geomap.face_textures.get(face_id).unwrap();
            let texture_name = map_data.geomap.textures.get(texture_id).unwrap();

            if texture_name == "skip" || texture_name.starts_with("skip_") {
                continue;
            };

            let (content_flags, mut surface_flags, _value) = match &map_data
                .geomap
                .face_extensions
                .get(face_id)
                .unwrap_or(&shalrath::repr::Extension::Standard)
            {
                &shalrath::repr::Extension::Quake2 {
                    content_flags,
                    surface_flags,
                    value,
                } => (*content_flags, *surface_flags, *value),
                _ => (0, 0, 0.0),
            };

            let vertices = &map_data.face_vertices.get(&face_id).unwrap();

            let mut use_inverted_tris = false;

            if content_flags & surfaces::NiBroomContent::Sky as u32 == 1 {
                use_inverted_tris = true;
            }

            if content_flags & surfaces::NiBroomContent::UseEmissive as u32 != 0 {
                node.use_emissive = true;
            }

            let mut indices = if use_inverted_tris {
                map_data.inverted_face_tri_indices.get(&face_id).unwrap_or_else(|| {
panic!("Critical error: Missing inverted face triangle indices for face_id: {:?}", face_id)
}).clone()
            } else {
                map_data
                    .face_tri_indices
                    .get(&face_id)
                    .unwrap_or_else(|| {
                        panic!(
                            "Critical error: Missing face triangle indices for face_id: {:?}",
                            face_id
                        )
                    })
                    .clone()
            };

            // We can't do fuzzier matches on this, so,
            // we'll have to hardcode a set of sky texture names (Thanks skyrim)
            if texture_name.to_ascii_lowercase() == "sky5_blu" {
                node.use_emissive = true;
            }

            // Test for water or slime types
            if texture_name.to_ascii_lowercase().contains("slime")
                || texture_name.to_ascii_lowercase().contains("water")
                || texture_name.to_ascii_lowercase().contains("lava")
                || texture_name.to_ascii_lowercase().contains("mwat")
            {
                let inverted_indices = map_data.inverted_face_tri_indices.get(&face_id).unwrap();
                indices.extend_from_slice(inverted_indices);

                surface_flags |= surfaces::NiBroomSurface::NoClip as u32;
                println!("{face_id} interpreted as liquid")
            }

            let uv_sets = &map_data
                .face_uvs
                .get(&face_id)
                .expect("Unable to collect face UVs for {face_id}");

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
            if surface_flags & surfaces::NiBroomSurface::NoClip as u32 == 0 {
                node.col_verts.extend(*vertices);
                node.col_tris.push((*indices).to_vec());
            }
        }
        node
    }

    fn collect_faces_with_textures(brush_id: &BrushId, map_data: &MapData) -> Vec<Vec<FaceId>> {
        let mut face_textures = Vec::new();

        let faces = map_data.geomap.brush_faces.get(brush_id).unwrap();

        for face in faces.iter() {
            let texture_id = map_data.geomap.face_textures.get(face).unwrap();
            let texture_name = map_data.geomap.textures.get(texture_id).unwrap();
            if !face_textures.contains(texture_name) {
                face_textures.push(texture_name.to_string())
            }
        }

        let mut faces_with_matching_textures: Vec<Vec<FaceId>> =
            vec![Vec::new(); face_textures.len()];

        for (index, texture) in face_textures.iter().enumerate() {
            for face in faces.iter() {
                let texture_id = map_data.geomap.face_textures.get(face).unwrap();
                let texture_name = map_data.geomap.textures.get(texture_id).unwrap();

                if texture_name == texture {
                    faces_with_matching_textures[index].push(*face);
                }
            }
        }

        faces_with_matching_textures
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
        if self.vis_verts.len() > 0 {
            self.distance_from_origin = Mesh::centroid(&self.vis_verts)
        }

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
            use_emissive: false,
            normals: Vec::new(),
            texture: String::new(),
            uv_sets: Vec::new(),
            col_shape: NiTriShape::default(),
            col_data: NiTriShapeData::default(),
            col_verts: Vec::new(),
            col_tris: Vec::new(),
            distance_from_origin: SV3::default(),
        }
    }
}
