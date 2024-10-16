use nalgebra::{Rotation3, Vector3};
use openmw_cfg::{find_file, get_config};
use shambler::{brush::BrushId, Vector3 as SV3};
use tes3::{
    esp,
    nif::{self, NiLink, NiMaterialProperty, NiNode, NiStream, NiTriShapeData, RootCollisionNode},
};

use crate::{surfaces, BrushNiNode, MapData};

#[derive(Clone)]
pub struct Mesh {
    pub game_object: esp::TES3Object,
    pub node_distances: Vec<SV3>,
    pub stream: NiStream,
    pub base_index: NiLink<NiNode>,
    pub final_distance: SV3,
    pub mangle: [f32; 3],
    collision_index: NiLink<RootCollisionNode>,
}

impl Mesh {
    fn new(scale_mode: &f32) -> Self {
        let mut stream = NiStream::default();
        let mut root_node = NiNode::default();
        let mut base_node = NiNode::default();
        let collision_index = stream.insert(RootCollisionNode::default());
        base_node.children.push(collision_index.cast());
        base_node.scale = *scale_mode;
        let base_index = stream.insert(base_node);
        root_node.children.push(base_index.cast());
        let root_index = stream.insert(root_node);

        stream.roots = vec![root_index.cast()];

        Mesh {
            stream,
            base_index,
            collision_index,
            game_object: esp::TES3Object::Static(esp::Static::default()),
            node_distances: Vec::new(),
            final_distance: SV3::default(),
            mangle: [0.0, 0.0, 0.0],
        }
    }

    pub fn from_map(brushes: &Vec<BrushId>, map_data: &MapData, scale_mode: &f32) -> Mesh {
        let mut mesh = Mesh::new(scale_mode);

        for brush_id in brushes {
            let brush_nodes = BrushNiNode::from_brush(brush_id, map_data);

            for node in brush_nodes {
                mesh.attach_node(node);
            }
        }
        mesh
    }

    pub fn align_to_center(&mut self) {
        let center = Mesh::centroid(&self.node_distances);
        let rotation = Rotation3::new(Vector3::new(
            -self.mangle[0],
            -self.mangle[1],
            -self.mangle[2],
        ));
        for tri_shape in self.stream.objects_of_type_mut::<NiTriShapeData>() {
            for vert in &mut tri_shape.vertices {
                vert.x -= center.x;
                vert.y -= center.y;
                vert.z -= center.z;
                let rotated_vert = rotation.transform_vector(&Vector3::new(vert.x, vert.y, vert.z));
                vert.x = rotated_vert[0];
                vert.y = rotated_vert[1];
                vert.z = rotated_vert[2];
            }
        }

        for vert in &mut self.node_distances {
            vert.x -= center.x;
            vert.y -= center.y;
            vert.z -= center.z;
            let rotated_vert = rotation.transform_vector(&Vector3::new(vert.x, vert.y, vert.z));
            vert.x = rotated_vert[0];
            vert.y = rotated_vert[1];
            vert.z = rotated_vert[2];
        }
    }

    pub fn save(&mut self, name: &String) {
        self.align_to_center();
        let _ = self.stream.save_path(name);
    }

    pub fn centroid(vertices: &Vec<SV3>) -> SV3 {
        // Calculate the sum of all dimensions using fold
        vertices
            .iter()
            .fold(SV3::default(), |acc, v| acc + *v)
            .scale(1.0 / vertices.len() as f32)
    }

    pub fn attach_node(&mut self, node: BrushNiNode) {
        // HACK: This only gets used if the vis data and collision data are equal, so is always initialized when used
        let mut vis_data_index = NiLink::default();

        if node.vis_verts.len() > 0 {
            self.node_distances.push(node.distance_from_origin);

            let vis_index = self.stream.insert(node.vis_shape);

            self.assign_base_texture(vis_index, node.texture);

            if node.use_emissive {
                self.assign_material(vis_index)
            }

            vis_data_index = self.stream.insert(node.vis_data);

            if let Some(shape) = self.stream.get_mut(vis_index) {
                shape.geometry_data = vis_data_index.cast();
            };

            if let Some(root) = self.stream.get_mut(self.base_index) {
                root.children.push(vis_index.cast());
            };
        }

        if node.col_verts.len() > 0 {
            let col_index = self.stream.insert(node.col_shape);

            //HACK: We should probably implement equality traits instead of checking the length of the vertices, but it works
            let col_data_index = match node.col_verts.len() == node.vis_verts.len() {
                true => vis_data_index,
                false => self.stream.insert(node.col_data),
            };

            if let Some(collision) = self.stream.get_mut(col_index) {
                collision.geometry_data = col_data_index.cast();
            };

            if let Some(collision_root) = self.stream.get_mut(self.collision_index) {
                collision_root.children.push(col_index.cast());
            };
        }
    }

    fn assign_base_texture(&mut self, object: nif::NiLink<nif::NiTriShape>, file_path: String) {
        let config =
            get_config().expect("Openmw.cfg not located! Be sure you have a valid openmw setup.");
        // Create and insert a NiTexturingProperty and NiSourceTexture.
        let tex_prop_link = self.stream.insert(nif::NiTexturingProperty::default());
        let texture_link = self.stream.insert(nif::NiSourceTexture::default());

        let mut extension = String::default();

        for extension_candidate in ["png", "dds", "tga"] {
            let candidate_path = format!("Textures/{file_path}.{extension_candidate}");
            if let Ok(_) = find_file(&config, candidate_path.as_str()) {
                extension = extension_candidate.to_string();
                break;
            }
        }

        // Update the base map texture.
        let tex_prop = self.stream.get_mut(tex_prop_link).unwrap();
        tex_prop.texture_maps.resize(7, None); // not sure why
        let mut base_map = nif::Map::default();
        base_map.texture = texture_link.cast();
        tex_prop.texture_maps[0] = Some(nif::TextureMap::Map(base_map));

        // Update the texture source path.
        let texture = self.stream.get_mut(texture_link).unwrap();
        texture.source = nif::TextureSource::External(format!("{file_path}.{extension}").into());

        // Assign the tex prop to the target object
        let object = self.stream.get_mut(object).unwrap();
        object.properties.push(tex_prop_link.cast());
    }

    pub fn assign_material(&mut self, object: nif::NiLink<nif::NiTriShape>) {
        let mut mat = NiMaterialProperty {
            emissive_color: surfaces::colors::SKY.into(),
            ..Default::default()
        };

        mat.flags = 1;

        let mat_link = self.stream.insert(mat);

        // Assign the tex prop to the target object
        let object = self.stream.get_mut(object).unwrap();
        object.properties.push(mat_link.cast());
    }
}
