use openmw_cfg::{find_file, get_config};
use shambler::{brush::BrushId, Vector3 as SV3};
use tes3::{
    esp,
    nif::{self, NiLink, NiMaterialProperty, NiNode, NiStream, RootCollisionNode},
};

use crate::{surfaces, BrushNiNode, MapData};

pub struct Mesh {
    pub game_object: esp::TES3Object,
    pub node_distances: Vec<SV3>,
    pub stream: NiStream,
    base_index: NiLink<NiNode>,
    collision_index: NiLink<RootCollisionNode>,
    final_distance: SV3,
    root_index: NiLink<NiNode>,
}

impl Mesh {
    fn new() -> Self {
        let mut stream = NiStream::default();
        let mut root_node = NiNode::default();
        let mut base_node = NiNode::default();
        let collision_index = stream.insert(RootCollisionNode::default());
        base_node.children.push(collision_index.cast());
        base_node.scale = surfaces::MAP_SCALE; // Trenchbroom maps tend to be a bit small.
        let base_index = stream.insert(base_node);
        root_node.children.push(base_index.cast());
        let root_index = stream.insert(root_node);

        stream.roots = vec![root_index.cast()];

        Mesh {
            stream,
            root_index,
            base_index,
            collision_index,
            game_object: esp::TES3Object::Static(esp::Static::default()),
            node_distances: Vec::new(),
            final_distance: SV3::default(),
        }
    }

    pub fn from_map(brushes: &Vec<BrushId>, map_data: &MapData) -> Mesh {
        let mut mesh = Mesh::new();

        for brush_id in brushes {
            let brush_nodes = BrushNiNode::from_brush(brush_id, map_data);

            for node in brush_nodes {
                mesh.attach_node(node);
            }
        }
        mesh
    }

    pub fn save(&mut self, name: &String) {
        let _ = self.stream.save_path(name);
    }

    pub fn attach_node(&mut self, node: BrushNiNode) {
        // HACK: This only gets used if the vis data and collision data are equal, so is always initialized when used
        let mut vis_outer = NiLink::default();

        if node.vis_verts.len() > 0 {
            self.node_distances.push(node.distance_from_origin);

            let vis_index = self.stream.insert(node.vis_shape);

            if !node.is_sky {
                self.assign_base_texture(vis_index, node.texture);
            } else {
                self.assign_material(vis_index);
            }

            let vis_data_index = self.stream.insert(node.vis_data);

            vis_outer = vis_data_index;

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
                true => vis_outer,
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

        for extension_candidate in ["dds", "tga"] {
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
