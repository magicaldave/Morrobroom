use crate::surfaces;
use std::collections::HashMap;
use tes3::esp::{
    Activator, Alchemy, AlchemyData, AlchemyFlags, Apparatus, ApparatusData, Armor, ArmorData,
    AtmosphereData, AttributeId, AttributeId2, BipedObject, Book, BookData, BookType, Cell,
    CellFlags, Effect, EffectId, EffectId2, EffectRange, Ingredient, IngredientData, Light,
    LightData, LightFlags, ObjectFlags, SkillId, SkillId2, TES3Object,
};

pub fn activator(
    entity_props: &HashMap<&String, &String>,
    ref_id: &str,
    mesh_name: &str,
) -> TES3Object {
    TES3Object::Activator(Activator {
        id: ref_id.to_owned(),
        name: get_prop("Name", entity_props),
        script: get_prop("Script", entity_props),
        mesh: mesh_name.to_owned(),
        ..Default::default()
    })
}

pub fn apparatus(
    entity_props: &HashMap<&String, &String>,
    ref_id: &str,
    mesh_name: &str,
) -> TES3Object {
    TES3Object::Apparatus(Apparatus {
        id: ref_id.to_owned(),
        name: get_prop("Name", entity_props),
        script: get_prop("Script", entity_props),
        mesh: mesh_name.to_owned(),
        data: ApparatusData {
            weight: get_prop("Weight", entity_props)
                .parse::<f32>()
                .unwrap_or_default(),
            value: get_prop("Value", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            quality: get_prop("Quality", entity_props)
                .parse::<f32>()
                .unwrap_or_default(),
            apparatus_type: get_prop("ApparatusType", entity_props)
                .parse::<u32>()
                .unwrap_or_default()
                .try_into()
                .expect("Invalid Apparatus Type!"),
        },
        ..Default::default()
    })
}

pub fn armor(
    entity_props: &HashMap<&String, &String>,
    ref_id: &str,
    mesh_name: &str,
) -> TES3Object {
    TES3Object::Armor(Armor {
        flags: ObjectFlags::default(),
        id: ref_id.to_owned(),
        name: get_prop("Name", entity_props),
        script: get_prop("Script", entity_props),
        mesh: mesh_name.to_owned(),
        icon: get_prop("Icon", entity_props),
        enchanting: get_prop("Enchantment", entity_props),
        biped_objects: collect_biped_objects(entity_props),
        data: ArmorData {
            armor_type: get_prop("ArmorType", entity_props)
                .parse::<u32>()
                .unwrap_or_default()
                .try_into()
                .expect("Invalid Armor Type!"),
            armor_rating: get_prop("ArmorRating", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            weight: get_prop("Weight", entity_props)
                .parse::<f32>()
                .unwrap_or_default(),
            value: get_prop("Value", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            health: get_prop("Health", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            enchantment: get_prop("EnchantmentPoints", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
        },
    })
}

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

pub fn cell(entity_props: &HashMap<&String, &String>) -> Cell {
    let mut flags = CellFlags::default() | CellFlags::IS_INTERIOR;

    flags |= [
        ("FakeExterior", CellFlags::BEHAVES_LIKE_EXTERIOR),
        ("RestIsIllegal", CellFlags::RESTING_IS_ILLEGAL),
        ("HasWater", CellFlags::HAS_WATER),
    ]
    .iter()
    .fold(CellFlags::empty(), |acc, &(prop, flag)| {
        acc | match get_prop(prop, entity_props).parse::<u32>() {
            Ok(1) => flag,
            _ => CellFlags::empty(),
        }
    });

    Cell {
        flags: ObjectFlags::default(),
        name: get_prop("Name", entity_props),
        data: tes3::esp::CellData {
            flags,
            grid: (0, 0),
        },
        region: match get_prop("Region", entity_props) {
            s if s == String::default() => None,
            _ => Some(get_prop("Region", entity_props)),
        },
        water_height: match flags & CellFlags::HAS_WATER {
            CellFlags::HAS_WATER => Some(
                get_prop("WaterHeight", entity_props)
                    .parse::<f32>()
                    .unwrap_or_default(),
            ),
            _ => None,
        },
        atmosphere_data: Some(AtmosphereData {
            fog_density: get_prop("FogDensity", entity_props)
                .parse::<f32>()
                .unwrap_or_default()
                .max(1.0)
                .min(0.0),
            fog_color: get_color(&get_prop("Fog_color", entity_props)),
            ambient_color: get_color(&get_prop("Ambient_color", entity_props)),
            sunlight_color: get_color(&get_prop("Sun_color", entity_props)),
        }),
        ..Default::default()
    }
}

pub fn ingredient(
    entity_props: &HashMap<&String, &String>,
    ref_id: &str,
    mesh_name: &str,
) -> TES3Object {
    let base_effects = collect_effects(entity_props, 4);
    let mut effects = [EffectId::None; 4];
    let mut attributes = [AttributeId::None; 4];
    let mut skills = [SkillId::None; 4];

    for (index, effect) in base_effects.iter().enumerate() {
        effects[index] = EffectId::try_from(effect.magic_effect as i32).expect("Cursed Toddism");
        match effect.magic_effect {
            EffectId2::DrainAttribute
            | EffectId2::DamageAttribute
            | EffectId2::AbsorbAttribute
            | EffectId2::FortifyAttribute
            | EffectId2::RestoreAttribute => {
                attributes[index] =
                    AttributeId::try_from(effect.attribute as i32).expect("Cursed Toddism");
            }
            EffectId2::DrainSkill
            | EffectId2::DamageSkill
            | EffectId2::AbsorbSkill
            | EffectId2::FortifySkill
            | EffectId2::RestoreSkill => {
                skills[index] = SkillId::try_from(effect.skill as i32).expect("Cursed Toddism");
            }
            _ => (),
        }
    }

    TES3Object::Ingredient(Ingredient {
        id: ref_id.to_owned(),
        name: get_prop("Name", entity_props),
        script: get_prop("Script", entity_props),
        mesh: mesh_name.to_owned(),
        data: IngredientData {
            weight: get_prop("Weight", entity_props)
                .parse::<f32>()
                .unwrap_or_default(),
            value: get_prop("Value", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            effects,
            attributes,
            skills,
        },
        ..Default::default()
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
            color: get_color(&get_prop("light_color", entity_props)),
        },
    })
}

pub fn potion(
    entity_props: &HashMap<&String, &String>,
    ref_id: &str,
    mesh_name: &str,
) -> TES3Object {
    TES3Object::Alchemy(Alchemy {
        flags: ObjectFlags::default(),
        id: ref_id.to_owned(),
        name: get_prop("Name", entity_props),
        script: get_prop("Script", entity_props),
        icon: get_prop("Icon", entity_props),
        mesh: mesh_name.to_owned(),
        data: AlchemyData {
            weight: get_prop("Weight", entity_props)
                .parse::<f32>()
                .unwrap_or_default(),
            value: get_prop("Value", entity_props)
                .parse::<u32>()
                .unwrap_or_default(),
            flags: AlchemyFlags::from_bits(
                get_prop("PotionFlags", entity_props)
                    .parse::<u32>()
                    .unwrap_or_default(),
            )
            .expect("Invalid Potion Flags!"),
        },
        effects: collect_effects(entity_props, 8),
    })
}

fn collect_effects(prop_map: &HashMap<&String, &String>, effects_size: u8) -> Vec<Effect> {
    let mut effects: Vec<Effect> = vec![];

    for count in 1..=effects_size {
        let effect_type = prop_map
            .get(&format!("Effect_{count}_MagicType"))
            .unwrap_or(&&String::default())
            .parse::<i16>()
            .unwrap_or(-1);

        match effect_type {
            -1 => continue, // Not 100% sure if this is valid but I'm fairly certain one
            // can't have a magic effect with no effect type
            _ => {
                let magnitude = prop_map
                    .get(&format!("Effect_{count}_Magnitude"))
                    .map(|s| s.parse::<u32>().unwrap_or_default());

                let (min_magnitude, max_magnitude) = match magnitude {
                    Some(mag) => (mag, mag),
                    None => (
                        prop_map
                            .get(&format!("Effect_{count}_MagnitudeMin"))
                            .map(|s| s.parse::<u32>().unwrap_or_default())
                            .unwrap_or_default(),
                        prop_map
                            .get(&format!("Effect_{count}_MagnitudeMax"))
                            .map(|s| s.parse::<u32>().unwrap_or_default())
                            .unwrap_or_default(),
                    ),
                };

                effects.push(Effect {
                    magic_effect: effect_type.try_into().expect("Invalid Magic Effect Type!"),
                    skill: SkillId2::try_from(match effect_type {
                        21 | 26 | 78 | 83 | 89 => {
                            // These are the skill effects
                            prop_map
                                .get(&format!("Effect_{count}_Skill"))
                                .unwrap_or(&&String::default())
                                .parse::<i8>()
                                .unwrap_or_default()
                        }
                        _ => -1,
                    })
                    .expect("Invalid Skill ID!"),
                    attribute: AttributeId2::try_from(match effect_type {
                        17 | 22 | 74 | 79 | 85 => {
                            // These are the attribute effects
                            prop_map
                                .get(&format!("Effect_{count}_Attribute"))
                                .unwrap_or(&&String::default())
                                .parse::<i8>()
                                .unwrap_or_default()
                        }
                        _ => -1,
                    })
                    .expect("Invalid Attribute ID!"),
                    range: EffectRange::try_from(
                        prop_map
                            .get(&format!("Effect_{count}_Range"))
                            .unwrap_or(&&String::default())
                            .parse::<u32>()
                            .unwrap_or_default(),
                    )
                    .expect("Invalid Effect Range!"),
                    area: prop_map
                        .get(&format!("Effect_{count}_Area"))
                        .unwrap_or(&&String::default())
                        .parse::<u32>()
                        .unwrap_or_default(),
                    duration: prop_map
                        .get(&format!("Effect_{count}_Duration"))
                        .unwrap_or(&&String::default())
                        .parse::<u32>()
                        .unwrap_or_default(),
                    min_magnitude,
                    max_magnitude,
                });
            }
        }
    }
    effects
}

fn collect_biped_objects(prop_map: &HashMap<&String, &String>) -> Vec<BipedObject> {
    let mut biped_objects = Vec::new();

    for count in 1..7 {
        match prop_map.get(&format!("SlotType{count}")) {
            Some(biped_object) => biped_objects.push(BipedObject {
                biped_object_type: biped_object
                    .parse::<u8>()
                    .unwrap_or_default()
                    .try_into()
                    .expect("Invalid Biped Object Type!"),
                male_bodypart: prop_map
                    .get(&format!("male_part{count}"))
                    .unwrap_or(&&String::default())
                    .to_string(),
                female_bodypart: prop_map
                    .get(&format!("female_part{count}"))
                    .unwrap_or(&&String::default())
                    .to_string(),
            }),
            None => continue,
        }
    }

    biped_objects
}

fn get_color(color_str: &String) -> [u8; 4] {
    let mut array = [0; 4];
    let colors: Vec<&str> = color_str.split_whitespace().collect();

    for (index, color) in colors.iter().enumerate() {
        array[index] = color.parse::<u8>().unwrap_or_default();
    }
    array[3] = *array.iter().max().expect("No brightness value found!");
    array
}

fn get_prop(prop_name: &str, prop_map: &HashMap<&String, &String>) -> String {
    prop_map
        .get(&prop_name.to_string())
        .unwrap_or(&&String::default())
        .to_string()
}
