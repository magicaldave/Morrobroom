use std::{fmt::Write, path::PathBuf};

use tes3::esp::{EditorId, TES3Object};

use crate::fgd::{self, base};

pub trait ToFGDProp {
    fn to_fgd<S: AsRef<str>>(&self, description: S, default: S) -> String;
}

impl ToFGDProp for String {
    fn to_fgd<S: AsRef<str>>(&self, description: S, default: S) -> String {
        format!(
            "{} (string) : \"{}\" : \"{}\"",
            self,
            &description.as_ref(),
            &default.as_ref()
        )
    }
}

pub fn get_object_bounds(
    config_manager: &crate::fgd::ConfigurationManager,
    object: &TES3Object,
) -> Option<[i32; 6]> {
    let object_model = PathBuf::from("Meshes/").join(match object {
        TES3Object::Static(record) => record.mesh.to_string(),
        TES3Object::Activator(record) => record.mesh.to_string(),
        TES3Object::Script(_) => {
            return None;
        }
        _ => unimplemented!(),
    });

    if let Some(vfs_file) = config_manager.vfs.get_file(object_model) {
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

pub fn serialize_objects_as_fgd(
    config_manager: &crate::fgd::ConfigurationManager,
    fgd_string: &mut String,
) -> Result<(), std::fmt::Error> {
    let mut scripts: Vec<String> = config_manager
        .merged_objects
        .iter()
        .filter_map(|(_, object)| {
            if matches!(object, TES3Object::Script(_)) {
                Some(object.editor_id_ascii_lowercase().to_string())
            } else {
                None
            }
        })
        .collect();

    scripts.insert(0, String::default());

    // let mut scripted_choices = String::from(base::OBJECT_SCRIPTED_CHOICES);

    let scripted_choices = format!(
        "@BaseClass = object_script
[
    {}
]",
        base::write_choices(
            scripts,
            "Script_List",
            "Available mwscripts to attach to a GameObject",
            true,
        )?
    );

    write!(fgd_string, "{scripted_choices}")?;
    serialize_base_objects_to_fgd(fgd_string)?;

    for (_, object) in &config_manager.merged_objects {
        let bounds = get_object_bounds(config_manager, object);

        match object {
            TES3Object::Script(_) => continue,
            _ => serialize_object_to_fgd(object, bounds, fgd_string)?,
        }
    }

    Ok(())
}

pub fn serialize_object_to_fgd(
    object: &TES3Object,
    bounds: Option<[i32; 6]>,
    fgd_string: &mut String,
) -> Result<(), std::fmt::Error> {
    let (min_x, min_y, min_z, max_x, max_y, max_z) = match bounds {
        Some(arr) => (arr[0], arr[1], arr[2], arr[3], arr[4], arr[5]),
        None => (-10, -20, -10, 10, 20, 10),
    };

    let get_color = |(min, max): (i32, i32)| (min.abs() + max.abs()) % 255;

    let (r, g, b) = (
        get_color((min_x, max_x)),
        get_color((min_y, max_y)),
        get_color((min_z, max_z)),
    );

    let object_string = match object {
        TES3Object::Static(record) => {
            format!(
                "
@PointClass base(world_Base) size({min_x} {min_y} {min_z}, {max_x} {max_y} {max_z}) color({r} {g} {b}) = {} : \"{}\"
[
    {}
]
                ",
                record.editor_id_ascii_lowercase().replace(' ', "_"),
                record.id,
                base::ref_id_string(&record.editor_id_ascii_lowercase())
            )
        }
        TES3Object::Activator(record) => {
            format!(
                "
@PointClass base(world_Activator) size({min_x} {min_y} {min_z}, {max_x} {max_y} {max_z}) color({r} {g} {b}) = {} : \"{}\"
[
    {}
    {}
]
                ",
                record.editor_id_ascii_lowercase().replace(' ', "_"),
                record.id,
                base::ref_id_string(&record.editor_id_ascii_lowercase()),
                String::from("script").to_fgd("MWScript used on a gameObject. Check the Script_list field for available scripts", &record.script),
            )
        }
        TES3Object::Script(_) => {
            return Ok(());
        }
        _ => unimplemented!(),
    };

    writeln!(fgd_string, "{object_string}")
}

pub fn serialize_base_objects_to_fgd(fgd_string: &mut String) -> Result<(), std::fmt::Error> {
    writeln!(fgd_string, "{}", base::MATERIAL_BASE_DEF)?;
    writeln!(fgd_string, "{}", base::STATIC_BASE_DEF)?;
    writeln!(fgd_string, "{}", base::ACTIVATOR_BASE_DEF)?;
    Ok(())
}
