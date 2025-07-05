use std::{collections::BTreeMap, path::PathBuf};

use openmw_config::{ConfigError, OpenMWConfiguration};
use rayon::prelude::*;
use tes3::esp::{EditorId, Plugin, TES3Object};
use vfstool_lib::VFS;

mod brush_class_props;
/// To add a new record type to the serializer:
/// 1: Implement the ToFGDProp trait for that record type
/// 2: In src/serialize.rs, add the relevant fields to the `serialize_typed_objects_as_fgd` function
/// 3: Add that specific record type, to `serialize_object_as_fgd`, for the serialization of each individual record
/// 4: Add a test module for serializing that specific record type. Just copy and paste one of the existing ones at the bottom of this file and change the tag and the output file name.
/// 5: Add relevant bounds handling to `get_object_bounds_from_nif` and `get_object_model_path`
/// 6: Actually deserialize that record type during loading, in ConfigurationManager::collect_merged_objects
mod serialize;

#[derive(Debug)]
pub enum ConfigManagerError {
    OpenMWConfigError(ConfigError),
    MissingPluginErr(String),
}

impl std::fmt::Display for ConfigManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenMWConfigError(err) => write!(f, "Failed reading openmw.cfg chain: {err}"),
            Self::MissingPluginErr(plugin) => write!(f, "Failed to find plugin: {plugin}"),
        }
    }
}

impl std::error::Error for ConfigManagerError {}

impl From<ConfigError> for ConfigManagerError {
    fn from(err: ConfigError) -> ConfigManagerError {
        Self::OpenMWConfigError(err)
    }
}

use std::collections::HashMap;
type BoundsMap = HashMap<String, [i32; 6]>;
pub struct ConfigurationManager {
    merged_objects: BTreeMap<String, (TES3Object, String)>,
    object_bounds: BoundsMap,
    vfs: VFS,
    openmw_config: OpenMWConfiguration,
}

pub fn get_object_model_path(object: &TES3Object) -> Option<String> {
    let mesh = match object {
        TES3Object::Script(_) | TES3Object::StartScript(_) => {
            return None;
        }
        TES3Object::Static(record) => &record.mesh,
        TES3Object::Activator(record) => &record.mesh,
        TES3Object::Ingredient(record) => &record.mesh,
        TES3Object::Light(record) => &record.mesh,
        _ => unimplemented!(),
    };

    if mesh == &String::default() {
        None
    } else {
        Some(mesh.to_ascii_lowercase())
    }
}

fn get_object_bounds_from_nif(vfs: &VFS, object: &TES3Object) -> Option<[i32; 6]> {
    let object_model = PathBuf::from("Meshes/").join(match object {
        TES3Object::Static(record) => &record.mesh,
        TES3Object::Activator(record) => &record.mesh,
        TES3Object::Ingredient(record) => &record.mesh,
        TES3Object::Light(record) => {
            if record.mesh == String::default() {
                return None;
            } else {
                &record.mesh
            }
        }
        TES3Object::Script(_) => {
            return None;
        }
        _ => unimplemented!(),
    });

    if let Some(vfs_file) = vfs.get_file(&object_model) {
        if let Ok(stream) = tes3::nif::NiStream::from_path(vfs_file.path()) {
            if let Some((min, max)) = stream.bounding_box() {
                Some([
                    min.x as i32,
                    min.y as i32,
                    min.z as i32,
                    max.x as i32,
                    max.y as i32,
                    max.z as i32,
                ])
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

impl ConfigurationManager {
    pub fn collect_merged_objects(&mut self) -> Result<(), ConfigManagerError> {
        self.openmw_config
            .content_files()
            .par_iter()
            .rev()
            .map(|plugin_name| {
                if let Some(file) = self.vfs.get_file(plugin_name) {
                    if let Ok(plugin) = tes3::esp::Plugin::from_path_filtered(file.path(), |tag| {
                        matches!(
                            &tag,
                            tes3::esp::Static::TAG
                                | tes3::esp::Script::TAG
                                | tes3::esp::Activator::TAG
                                | tes3::esp::Ingredient::TAG
                                | tes3::esp::Light::TAG
                        )
                    }) {
                        Ok((plugin, plugin_name))
                    } else {
                        Err(plugin_name)
                    }
                } else {
                    Err(plugin_name)
                }
            })
            .collect::<Vec<Result<(Plugin, &&String), &&String>>>()
            .into_iter()
            .try_for_each(|plugin| match plugin {
                Err(missing_plugin) => Err(ConfigManagerError::MissingPluginErr(
                    missing_plugin.to_string(),
                )),
                Ok((plugin, plugin_name)) => {
                    plugin.objects.into_iter().for_each(|tes3_object| {
                        let editor_id = tes3_object.editor_id_ascii_lowercase().to_string();

                        if let Some(model) = get_object_model_path(&tes3_object) {
                            if let None = self.object_bounds.get(&model) {
                                if let Some(bounds) =
                                    get_object_bounds_from_nif(&self.vfs, &tes3_object)
                                {
                                    self.object_bounds.insert(model, bounds);
                                };
                            };
                        }

                        // Remember this won't work for cells :D
                        if !self.merged_objects.contains_key(&editor_id) {
                            self.merged_objects
                                .insert(editor_id, (tes3_object, plugin_name.to_ascii_lowercase()));
                        }
                    });
                    Ok(())
                }
            })
    }

    fn get_object_bounds(&self, mesh_path: &String) -> Option<&[i32; 6]> {
        self.object_bounds.get(mesh_path)
    }
}

impl TryFrom<&str> for ConfigurationManager {
    type Error = ConfigManagerError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let config_path = PathBuf::from(value);

        let openmw_config = OpenMWConfiguration::new(Some(config_path))?;

        let vfs = VFS::from_directories(
            openmw_config.data_directories(),
            Some(
                openmw_config
                    .fallback_archives()
                    .iter()
                    .map(|archive| archive.as_ref())
                    .collect(),
            ),
        );

        let mut manager = ConfigurationManager {
            object_bounds: HashMap::new(),
            vfs,
            openmw_config,
            merged_objects: BTreeMap::new(),
        };

        manager.collect_merged_objects()?;

        Ok(manager)
    }
}

#[cfg(test)]
mod cfgmgr_test {
    use std::{fs::File, io::BufWriter};

    use crate::fgd::{ConfigurationManager, serialize};

    #[test]
    fn test_default_path() {
        let path = openmw_config::default_config_path();
        assert!(ConfigurationManager::try_from(path.to_str().unwrap()).is_ok());
    }

    #[test]
    fn test_serialize_all() {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap()).unwrap();

        let mut file = File::create("./FGDOut.fgd").unwrap();
        let mut writer = BufWriter::new(&mut file);

        assert!(serialize::serialize_objects_as_fgd(&config, &mut writer).is_ok());
    }

    #[test]
    fn test_serialize_static() {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap()).unwrap();

        let mut file = File::create("./FGDOut_Static.fgd").unwrap();
        let mut writer = BufWriter::new(&mut file);

        assert!(
            serialize::serialize_typed_objects_as_fgd(
                &config,
                &mut writer,
                tes3::esp::Static::TAG_STR
            )
            .is_ok()
        );
    }

    #[test]
    fn test_serialize_activator() {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap()).unwrap();

        let mut file = File::create("./FGDOut_Activator.fgd").unwrap();
        let mut writer = BufWriter::new(&mut file);

        assert!(
            serialize::serialize_typed_objects_as_fgd(
                &config,
                &mut writer,
                tes3::esp::Activator::TAG_STR
            )
            .is_ok()
        );
    }

    #[test]
    fn test_serialize_script() {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap()).unwrap();

        let mut file = File::create("./FGDOut_Script.fgd").unwrap();
        let mut writer = BufWriter::new(&mut file);

        assert!(
            serialize::serialize_typed_objects_as_fgd(
                &config,
                &mut writer,
                tes3::esp::Script::TAG_STR
            )
            .is_ok()
        );
    }

    #[test]
    fn test_serialize_ingredient() {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap()).unwrap();

        let mut file = File::create("./FGDOut_Ingredient.fgd").unwrap();
        let mut writer = BufWriter::new(&mut file);

        assert!(
            serialize::serialize_typed_objects_as_fgd(
                &config,
                &mut writer,
                tes3::esp::Ingredient::TAG_STR
            )
            .is_ok()
        );
    }

    #[test]
    fn test_serialize_light() {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap()).unwrap();

        let mut file = File::create("./FGDOut_Light.fgd").unwrap();
        let mut writer = BufWriter::new(&mut file);

        assert!(
            serialize::serialize_typed_objects_as_fgd(
                &config,
                &mut writer,
                tes3::esp::Light::TAG_STR
            )
            .is_ok()
        );
    }
}
