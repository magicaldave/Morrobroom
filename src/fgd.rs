use std::collections::BTreeMap;

use openmw_config::{ConfigError, OpenMWConfiguration};
use rayon::prelude::*;
use tes3::esp::{EditorId, Plugin, TES3Object};
use vfstool_lib::VFS;

mod base;
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

pub struct ConfigurationManager {
    merged_objects: BTreeMap<String, TES3Object>,
    vfs: VFS,
    openmw_config: OpenMWConfiguration,
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
                        )
                    }) {
                        Ok(plugin)
                    } else {
                        Err(plugin_name)
                    }
                } else {
                    Err(plugin_name)
                }
            })
            .collect::<Vec<Result<Plugin, &&String>>>()
            .into_iter()
            .try_for_each(|plugin| match plugin {
                Err(missing_plugin) => Err(ConfigManagerError::MissingPluginErr(
                    missing_plugin.to_string(),
                )),
                Ok(plugin) => {
                    plugin.objects.into_iter().for_each(|tes3_object| {
                        let editor_id = tes3_object.editor_id_ascii_lowercase().to_string();

                        // Remember this won't work for cells :D
                        if !self.merged_objects.contains_key(&editor_id) {
                            eprintln!("Inserting {editor_id}");
                            self.merged_objects.insert(editor_id, tes3_object);
                        }
                    });
                    Ok(())
                }
            })
    }
}

use std::path::PathBuf;
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
            vfs,
            openmw_config,
            merged_objects: BTreeMap::new(),
        };

        manager.collect_merged_objects()?;

        // manager.merged_objects;

        Ok(manager)
    }
}

#[cfg(test)]
mod cfgmgr_test {
    use std::io::Write;

    use crate::fgd::{ConfigManagerError, ConfigurationManager};

    #[test]
    fn test_default_path() -> Result<(), ConfigManagerError> {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap())?;

        dbg!(&config.merged_objects);

        Ok(())
    }

    #[test]
    fn test_serialize() -> Result<(), ConfigManagerError> {
        let path = openmw_config::default_config_path();
        let config = ConfigurationManager::try_from(path.to_str().unwrap())?;
        let mut string = String::new();

        crate::fgd::serialize::serialize_objects_as_fgd(&config, &mut string).unwrap();

        eprintln!("{string}",);

        let mut out_fgd = std::fs::File::create("./FGDOut.fgd").unwrap();

        out_fgd.write(string.as_bytes()).unwrap();

        Ok(())
    }
}
