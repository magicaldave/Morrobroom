use std::{
    borrow::Cow,
    collections::{
        hash_map::DefaultHasher,
        {BTreeMap, BTreeSet},
    },
    hash::{Hash, Hasher},
    io::{self, Write},
};

use tes3::esp::{EditorId, ObjectFlags, TES3Object, TypeInfo};

pub mod std_write_fgd;
use std_write_fgd::STDWriteFGD;

pub mod write_fgd_prop;
use write_fgd_prop::WriteFGDProp;

type PluginRecordMap<'a> = BTreeMap<&'a String, BTreeSet<Cow<'a, str>>>;

/// Generates a unique but deterministic RGB color based on the input ID string.
/// Returns [i32; 3] with all values in the range 0..=255.
fn generate_rgb_from_id(id: &str) -> [i32; 3] {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();

    // Split hash into 3 bytes for RGB
    [
        ((hash & 0xFF) as u8) as i32,
        (((hash >> 8) & 0xFF) as u8) as i32,
        (((hash >> 16) & 0xFF) as u8) as i32,
    ]
}

pub fn write_base_class<W: Write, D: std::fmt::Display>(
    fgd_string: &mut W,
    class_name: D,
) -> Result<(), io::Error> {
    writeln!(fgd_string, "@BaseClass = {}\n[", class_name)
}

pub fn write_point_class<W: Write>(
    fgd_string: &mut W,
    classes: &[&str],
    bounds: Option<&[i32; 6]>,
    id: String,
) -> Result<(), io::Error> {
    write!(fgd_string, "@PointClass")?;

    write_fgd_base(fgd_string, classes)?;

    let color = write_fgd_size(bounds, fgd_string)?;

    write_fgd_color(&color, fgd_string)?;

    writeln!(fgd_string, " = {id}")?;

    Ok(())
}

pub fn write_unplaceable_point_class<W: Write>(
    fgd_string: &mut W,
    classes: &[&str],
    bounds: Option<&[i32; 6]>,
    color: &[i32; 3],
    id: String,
) -> Result<(), io::Error> {
    write!(fgd_string, "@PointClass")?;

    write_fgd_base(fgd_string, classes)?;

    write_fgd_size(bounds, fgd_string)?;

    write_fgd_color(color, fgd_string)?;

    writeln!(fgd_string, " = {id}")?;

    Ok(())
}

pub fn write_light_point_class<W: Write>(
    fgd_string: &mut W,
    classes: &[&str],
    bounds: &[i32; 6],
    color: &[u8; 4],
    id: String,
) -> Result<(), io::Error> {
    write!(fgd_string, "@PointClass")?;

    write_fgd_base(fgd_string, classes)?;

    write_fgd_size(Some(bounds), fgd_string)?;

    write_light_fgd_color(&color, fgd_string)?;

    writeln!(fgd_string, " = {id}")?;

    Ok(())
}

pub fn write_fgd_size<W: Write>(
    bounds: Option<&[i32; 6]>,
    fgd_string: &mut W,
) -> Result<[i32; 3], io::Error> {
    let Some([min_x, min_y, min_z, max_x, max_y, max_z]) = bounds else {
        return Ok([255, 255, 255]);
    };

    write!(
        fgd_string,
        " size({min_x} {min_y} {min_z}, {max_x} {max_y} {max_z})"
    )?;

    let get_color = |(min, max): (i32, i32)| (min.abs() + max.abs()) % 255;

    let (r, g, b) = (
        get_color((*min_x, *max_x)),
        get_color((*min_y, *max_y)),
        get_color((*min_z, *max_z)),
    );

    Ok([r, g, b])
}

pub fn write_fgd_color<W: Write>(color: &[i32; 3], fgd_string: &mut W) -> Result<(), io::Error> {
    let (r, g, b) = (color[0], color[1], color[2]);
    write!(fgd_string, " color({r} {g} {b})")
}

pub fn write_light_fgd_color<W: Write>(
    color: &[u8; 4],
    fgd_string: &mut W,
) -> Result<(), io::Error> {
    let (r, g, b) = (color[0], color[1], color[2]);
    write!(fgd_string, " color({r} {g} {b})")
}

pub fn write_fgd_base<W: Write>(fgd_string: &mut W, classes: &[&str]) -> io::Result<()> {
    if !classes.is_empty() {
        let mut buffer = String::with_capacity(64);

        buffer.push_str(" base(");
        buffer.push_str(&classes.join(",")); // simple, readable
        buffer.push(')');

        write!(fgd_string, "{buffer}")?;
    }

    Ok(())
}

pub fn write_object_flags<W: Write>(fgd_string: &mut W, flags: &ObjectFlags) -> io::Result<()> {
    "ObjectFlags".write_fgd(
        fgd_string,
        "",
        &flags
            .iter_names()
            .map(|(name, _)| name)
            .collect::<Vec<_>>()
            .join(" | "),
    )
}

pub fn serialize_objects_as_fgd<W: Write>(
    config_manager: &crate::fgd::ConfigurationManager,
    fgd_string: &mut W,
) -> Result<(), std::io::Error> {
    write_dictionary(&config_manager.merged_objects, fgd_string)?;

    serialize_base_object_to_fgd(fgd_string)?;

    for (_, (object, parent_plugin)) in &config_manager.merged_objects {
        let bounds = if let Some(path) = crate::fgd::get_object_model_path(object) {
            config_manager.get_object_bounds(&path)
        } else {
            None
        };

        match object {
            TES3Object::Script(_) => continue,
            _ => serialize_object_to_fgd(object, bounds, parent_plugin, fgd_string)?,
        }
    }

    Ok(())
}

pub fn serialize_typed_objects_as_fgd<W: Write>(
    config_manager: &crate::fgd::ConfigurationManager,
    fgd_string: &mut W,
    object_type: &'static str,
) -> Result<(), std::io::Error> {
    serialize_base_object_to_fgd(fgd_string)?;

    let (class_name, list_name, list_desc, object_type) = match object_type {
        "STAT" => (
            "StaticList",
            "static_list",
            "List of all static objects in this configuration",
            tes3::esp::Static::TAG,
        ),
        "ACTI" => (
            "ActivatorList",
            "activator_list",
            "List of all activators in this configuration",
            tes3::esp::Activator::TAG,
        ),
        "SCPT" => (
            "ScriptList",
            "script_list",
            "List of all mwscripts in this configuration",
            tes3::esp::Script::TAG,
        ),
        "INGR" => (
            "IngredientList",
            "ingredient_list",
            "List of all ingredients in this configuration",
            tes3::esp::Ingredient::TAG,
        ),
        "LIGH" => (
            "LightList",
            "light_list",
            "List of all lights in this configuration",
            tes3::esp::Light::TAG,
        ),
        _ => unimplemented!(),
    };

    write_choice_list_by_tag(
        &config_manager.merged_objects,
        fgd_string,
        class_name,
        object_type,
        list_name,
        list_desc,
    )?;

    for (_, (object, parent_plugin)) in &config_manager.merged_objects {
        let bounds = if let Some(path) = crate::fgd::get_object_model_path(object) {
            config_manager.get_object_bounds(&path)
        } else {
            None
        };

        if object.tag() == object_type {
            serialize_object_to_fgd(object, bounds, parent_plugin, fgd_string)?
        }
    }

    Ok(())
}

pub fn serialize_object_to_fgd<W: Write>(
    object: &TES3Object,
    bounds: Option<&[i32; 6]>,
    parent_plugin: &String,
    fgd_string: &mut W,
) -> Result<(), io::Error> {
    match object {
        TES3Object::Static(record) => {
            record.write_fgd(fgd_string, parent_plugin, bounds)?;
        }
        TES3Object::Activator(record) => {
            record.write_fgd(fgd_string, parent_plugin, bounds)?;
        }
        TES3Object::Ingredient(record) => {
            record.write_fgd(fgd_string, parent_plugin, bounds)?;
        }
        TES3Object::Light(record) => {
            record.write_fgd(fgd_string, parent_plugin, bounds)?;
        }
        TES3Object::Script(_) => {}
        _ => unimplemented!(),
    };

    Ok(())
}

pub fn serialize_base_object_to_fgd<W: Write>(fgd_string: &mut W) -> Result<(), std::io::Error> {
    write_base_class(fgd_string, "world_Base")?;

    writeln!(
        fgd_string,
        "    mangle (string) : \"Object Rotation\" : \"0 0 0\"\n]\n"
    )
}

/// Write a choice list that groups TES3 records by owner plug‑in and displays them in an FGD
/// `choices` block.
///
/// The list syntax produced looks like:
///
/// ```fgd
///   ItemId(choices) : "Item list" : "" =
///   [
///       "example_id" : "myplugin.esp"
///       ...
///   ]
/// ```
pub fn write_choice_list_by_tag<W: io::Write>(
    merged_objects: &BTreeMap<String, (TES3Object, String)>,
    out: &mut W,
    class_name: &str,
    object_tag: &[u8; 4],
    list_name: &str,
    list_desc: &str,
) -> io::Result<()> {
    // Write base class declaration
    write_base_class(out, class_name)?;

    writeln!(
        out,
        "    {list_name}(choices) : \"{list_desc}\" : \"\" =\n    ["
    )?;

    // Build: plugin -> set of ids
    let mut by_plugin: BTreeMap<&str, BTreeSet<Cow<'_, str>>> = BTreeMap::new();

    for (object, owner_plugin) in merged_objects.values() {
        if object.tag() == object_tag {
            let id = object.editor_id_ascii_lowercase(); // Cow<'_, str>
            by_plugin
                .entry(owner_plugin.as_str())
                .or_default()
                .insert(id);
        }
    }

    // Emit each id
    for (plugin, ids) in by_plugin {
        for id in ids {
            writeln!(out, "        \"{id}\" : \"{plugin}\"")?;
        }
    }

    writeln!(out, "    ]\n]\n")
}

pub fn write_dictionary<W: Write>(
    merged_objects: &BTreeMap<String, (TES3Object, String)>,
    fgd_string: &mut W,
) -> Result<(), io::Error> {
    [
        (
            "ActivatorList",
            "activator_list",
            "Available activators from the provided openmw config. Activators are similar to statics, but may be scripted.",
            tes3::esp::Activator::TAG,
        ),
        ("StaticList", "static_list", "Available statics from the provided openmw config. Only for scene decoration.", tes3::esp::Static::TAG),
        ("ScriptList", "script_list", "Available MWScripts from the provided openmw config. May not be edited in TB.", tes3::esp::Script::TAG),
        ("IngredientList", "ingredient_list", "Available ingredients from the provided openmw config.", tes3::esp::Ingredient::TAG),
    ]
    .into_iter()
    .try_for_each(|(class_name, list_name, list_desc, object_type)| {
        write_choice_list_by_tag(
            merged_objects,
            fgd_string,
            class_name,
            object_type,
            list_name,
            list_desc,
        )
    })?;

    writeln!(
        fgd_string,
        "@PointClass base(ActivatorList, StaticList, ScriptList, IngredientList) size(-32 -32 -32, 32 32 32) color(255 0 255) = tool_Dictionary : \"Global dictionary of all non-referenceable records in your openmw installation.\" []\n"
    )?;

    Ok(())
}
