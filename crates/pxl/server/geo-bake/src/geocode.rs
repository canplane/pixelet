// pxl-geo-bake/geocode.rs

pub mod provider;

use std::collections::{BTreeMap, HashMap};

use xpute_core::status::error::MarshalError;

use pxl_spec::bake::place_page::{encode_place_dict, Division};

/// A place many pages share is carried by its dictionary id; one only a few
/// pages share is carried inline by its group.
pub enum PartSource {
    Division(String),
    Near(Division),
}

/// Read whole before any group is written: a label is a place's position in
/// this list, so every reader must see the same order.
pub struct PlaceDict {
    pub divisions: Vec<Division>,
    at: HashMap<String, u32>,
}

impl PlaceDict {
    pub fn of(rows: Vec<(String, Named)>) -> PlaceDict {
        let at = rows.iter().enumerate().map(|(i, (id, _))| (id.clone(), i as u32)).collect();
        PlaceDict {
            divisions: rows.into_iter().map(|(_, d)| d).collect(),
            at,
        }
    }

    pub fn number(&self, id: &str) -> Option<u32> {
        self.at.get(id).copied()
    }

    pub fn packet(&self) -> Result<Vec<u8>, MarshalError> {
        encode_place_dict(&self.divisions)
    }
}

pub fn names_of(common: &Option<serde_json::Value>) -> BTreeMap<String, String> {
    let Some(serde_json::Value::Object(map)) = common else { return BTreeMap::new() };
    map.iter().filter_map(|(lang, name)| name.as_str().map(|n| (lang.clone(), n.to_string()))).collect()
}

#[cfg(test)]
#[path = "geocode.test.rs"]
mod test;
