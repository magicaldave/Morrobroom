pub static STATIC_BASE_DEF: &'static str = r#"@PointClass base(material) = world_Base : "Base World Entity" [
    RefId (string) : "Object Record Id" : "Agronian Guy"
    mangle (string) : "Object Rotation" : "0 0 0"
]"#;

pub static CHOICE_ITEM_BASE: &'static str = r#""{CHOICE_NUM}" : "{CHOICE_NAME}""#;

pub static CHOICE_LIST: &'static str = r#"{LIST_NAME}(choices) : "{LIST_DESCRIPTION}": "NONE" =
    [
        {CHOICES}
    ]"#;

pub static OBJECT_SCRIPTED_CHOICES: &'static str = r#"@BaseClass = object_script
[
   {CHOICE_LIST} 
]"#;

pub static ACTIVATOR_BASE_DEF: &'static str = r#"@PointClass base(world_Base, object_script) = world_Activator : "Base Activator Entity" [
]"#;

pub static MATERIAL_BASE_DEF: &'static str = r#"@BaseClass = material
[
    Material_Emissive_color(color) : "Color emitted by the brush" : "1.0 0 0"
    Material_Ambient_color(color) : "Ambient color of the brush" : "0 1.0 0"
    Material_Diffuse_color(color) : "Diffuse color of the brush" : "0 0 1.0"

    Material_Alpha(float) : "Material Transparency" : "1.0"
	Material_Alpha_UseBlend(choices) : "Use alpha blending for this brush" : 0 =
    [
        0 : "False"
        1 : "True"
    ]
    Material_Alpha_BlendSourceMode(choices) : "Mode to use for alpha blending on the light source": 0 =
    [
        0 : "One"
        2 : "Zero"
        4 : "Source Color"
        6 : "One Minus Source Color"
        8 : "Destination Color"
        10 : "One Minus Destination Color"
        12 : "Source Alpha"
        14 : "One Minus Source Alpha"
        16 : "Destination Alpha"
        18 : "One Minus Destination Alpha"
        20 : "Source Alpha Saturate"
    ]
    Material_Alpha_BlendDestinationMode(choices) : "Mode to use for alpha blending on the light destination": 0 =
    [
        0 : "One"
        32 : "Zero"
        64 : "Source Color"
        96 : "One Minus Source Color"
        128 : "Destination Color"
        160 : "One Minus Destination Color"
        192 : "Source Alpha"
        224 : "One Minus Source Alpha"
        256 : "Destination Alpha"
        288 : "One Minus Destination Alpha"
        320 : "Source Alpha Saturate"
    ]

	Material_Alpha_TestEnable(choices) : "Use alpha testing for this brush" : 0 =
    [
        0 : "False"
        512 : "True"
    ]
	Material_Alpha_TestFunction(choices) : "Use alpha testing for this brush" : 0 =
    [
        0 : "Always"
        1024 : "Less"
        2048 : "Equal"
        3072 : "Less Than Or Equal"
        4096 : "Greater Than"
        5120 : "Not Equal"
        6144 : "Greater Than Or Equal"
        7168 : "Never"
    ]
    Material_Alpha_TestThreshold(float) : "Threshold to use against the background when alpha testing": "1.0"

	Material_Alpha_NoSort(choices) : "Disable triangle sorting for this object" : 0 =
    [
        0 : "False"
        8192 : "True"
    ]
]"#;

use crate::fgd::serialize::ToFGDProp;
pub fn default_mangle() -> String {
    String::from("mangle").to_fgd("\"Object Rotation\"", "\"0 0 0\"")
}

pub fn ref_id_string(ref_id: &str) -> String {
    String::from("RefId")
        .to_string()
        .to_fgd("Object RecordId", &format!("{}", ref_id))
}

use std::fmt::Write;
/// Given a vector of possible choices, writes them out to the choice_string as an FGD-compatible list
pub fn write_choices(
    choices: Vec<String>,
    name: &str,
    description: &str,
    list: bool,
) -> Result<String, std::fmt::Error> {
    let mut choice_list_string = String::new();
    let default = String::from("NONE");

    for (idx, choice) in choices.iter().enumerate() {
        let idx_string = idx.to_string();
        let single_choice_string = CHOICE_ITEM_BASE
            .replace(
                "{CHOICE_NUM}",
                if !list {
                    &idx_string
                } else {
                    if *choice != String::default() {
                        &choice
                    } else {
                        &default
                    }
                },
            )
            .replace("{CHOICE_NAME}", choice);

        if let Err(error) = writeln!(choice_list_string, "{}", single_choice_string) {
            return Err(error);
        }
    }

    Ok(CHOICE_LIST
        .replace("{CHOICES}", &choice_list_string)
        .replace("{LIST_NAME}", name)
        .replace("{LIST_DESCRIPTION}", description))
}
