use imagesize::size;
use openmw_cfg::{find_file, get_config, Ini};
use shalrath::repr::*;
use shambler::{
    entity::EntityId,
    face::{FaceNormals, FaceTriangleIndices, FaceUvs, FaceVertices},
    texture::TextureId,
    GeoMap, Textures,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
};

pub struct MapData {
    pub geomap: GeoMap,
    pub face_vertices: FaceVertices,
    pub face_tri_indices: FaceTriangleIndices,
    pub inverted_face_tri_indices: FaceTriangleIndices,
    pub flat_normals: FaceNormals,
    pub smooth_normals: FaceNormals,
    pub face_uvs: FaceUvs,
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
                println!(
                    "Mapping texture {0} with sizes: {1}, {2}",
                    texture_name, texture_size.width, texture_size.height
                );
                (
                    texture_name.as_str(),
                    (texture_size.width as u32, texture_size.height as u32),
                )
            })
            .collect();

        let mut modified_textures: BTreeMap<TextureId, String> = BTreeMap::new();

        for (texture_id, texture_name) in geomap.textures.iter() {
            for texture_path in &texture_paths {
                if texture_path
                    .to_ascii_lowercase()
                    .contains(&texture_name.to_ascii_lowercase())
                {
                    modified_textures.insert(*texture_id, texture_path.to_string());
                }
            }
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

        MapData {
            geomap,
            face_vertices,
            face_tri_indices,
            inverted_face_tri_indices,
            flat_normals,
            smooth_normals,
            face_uvs,
        }
    }

    pub fn collect_textures(textures: &Textures) -> HashSet<String> {
        textures
            .iter()
            .map(|(_, texture_name)| texture_name.to_string())
            .collect()
    }

    pub fn find_vfs_texture(name: &str, config: &Ini) -> Option<String> {
        let extensions = ["dds", "tga", "png"];

        Some(
            extensions
                .iter()
                .flat_map(|extension| {
                    println!("Searching for texture: {name}");
                    find_file(config, format!("Textures/{}.{}", name, extension).as_str())
                })
                .next()
                .expect("Texture not found! This map is using a texture which isn't in your openmw vfs!")
                .to_string_lossy()
                .to_string(),
        )
    }

    pub fn find_textures_in_vfs(textures: &HashSet<String>) -> HashSet<String> {
        let config = get_config().expect("Openmw.cfg not detected! Please ensure you have a valid openmw configuration file in the canonical system directory.");
        textures
            .iter()
            .filter_map(|texture_name| MapData::find_vfs_texture(&texture_name, &config))
            .collect()
    }

    pub fn get_entity_properties(&self, entity_id: &EntityId) -> HashMap<&String, &String> {
        let entity_properties = self.geomap.entity_properties.get(&entity_id);

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

// pub use crate::map_data::MapData;
