use std::collections::HashMap;

pub mod fgd;

pub fn get_prop<'a>(
    prop_name: &str,
    prop_map: &'a HashMap<&String, &String>,
) -> Option<&'a String> {
    prop_map
        .iter()
        .find(|(k, _)| k.as_str() == prop_name)
        .map(|(_, v)| *v)
}
