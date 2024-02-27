use crate::surfaces;
use std::collections::HashMap;
use tes3::esp::{
    Book, BookData, BookType, Light, LightData, LightFlags, ObjectFlags, SkillId, TES3Object,
};

pub fn book(entity_props: &HashMap<&String, &String>, ref_id: &str, mesh_name: &str) -> TES3Object {
    TES3Object::Book(Book {
        flags: ObjectFlags::default(),
        id: ref_id.to_owned(),
        name: get_prop("Name", entity_props),
        script: get_prop("Script", entity_props),
        mesh: mesh_name.to_owned(),
        icon: get_prop("Icon", entity_props),
        enchanting: get_prop("Enchantment", entity_props),
        text: surfaces::BOOK_START_DEFAULT.to_owned() + &get_prop("Text", entity_props) + "<BR>",
        data: BookData {
            weight: get_prop("Weight", entity_props)
                .parse::<f32>()
                .unwrap_or_default(),
            value: get_prop("Value", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            book_type: BookType::try_from(
                get_prop("BookType", entity_props)
                    .parse::<u32>()
                    .unwrap_or_default(),
            )
            .expect("Book type out of range!"),
            skill: SkillId::try_from(
                get_prop("Skill", entity_props)
                    .parse::<i32>()
                    .unwrap_or_default(),
            )
            .expect("Invalid Skill ID Provided!"),
            enchantment: get_prop("EnchantmentPoints", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
        },
    })
}

pub fn light(
    entity_props: &HashMap<&String, &String>,
    ref_id: &str,
    mesh_name: &str,
) -> TES3Object {
    TES3Object::Light(Light {
        flags: ObjectFlags::default(),
        id: ref_id.to_owned(),
        name: get_prop("Name", entity_props),
        script: get_prop("Script", entity_props),
        mesh: mesh_name.to_owned(),
        icon: get_prop("Icon", entity_props),
        sound: get_prop("Sound", entity_props),
        data: LightData {
            weight: get_prop("Weight", entity_props)
                .parse::<f32>()
                .unwrap_or_default(),
            value: get_prop("Value", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            time: get_prop("Time", entity_props)
                .parse::<i32>()
                .unwrap_or_default(),
            radius: get_prop("Radius", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            flags: LightFlags::from_bits(
                get_prop("LightFlags", entity_props)
                    .parse::<u32>()
                    .unwrap_or_default(),
            )
            .expect("This cannot fail"), // Famous last words
            color: get_color(
                &get_prop("color", entity_props),
                &get_prop("Transparency", entity_props),
            ),
        },
    })
}

fn get_prop(prop_name: &str, prop_map: &HashMap<&String, &String>) -> String {
    prop_map
        .get(&prop_name.to_string())
        .unwrap_or(&&String::default())
        .to_string()
}

fn get_color(color_str: &String, alpha_str: &String) -> [u8; 4] {
    let mut array = [0; 4];
    let colors: Vec<&str> = color_str.split_whitespace().collect();

    for (index, color) in colors.iter().enumerate() {
        array[index] = color.parse::<u8>().unwrap_or_default();
    }

    array[3] = alpha_str.parse::<u8>().unwrap_or_default();

    array
}
