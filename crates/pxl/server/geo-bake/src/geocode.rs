// pxl-geo-bake/geocode.rs

pub mod provider;

use std::collections::HashMap;

use xpute_core::status::error::MarshalError;

use pxl_spec::bake::place_page::{encode_place_dict, Named};

/// A place many pages share is carried by its dictionary id; one only a few
/// pages share is carried inline by its group.
pub enum PartSource {
    Shared(String),
    Local { place: Named, approx: bool },
}

/// Read whole before any group is written: a label is a place's position in
/// this list, so every reader must see the same order.
pub struct PlaceDict {
    pub places: Vec<Named>,
    at: HashMap<String, u32>,
}

impl PlaceDict {
    pub fn of(rows: Vec<(String, Named)>) -> PlaceDict {
        let at = rows.iter().enumerate().map(|(i, (id, _))| (id.clone(), i as u32)).collect();
        PlaceDict {
            places: rows.into_iter().map(|(_, d)| d).collect(),
            at,
        }
    }

    pub fn number(&self, id: &str) -> Option<u32> {
        self.at.get(id).copied()
    }

    pub fn packet(&self) -> Result<Vec<u8>, MarshalError> {
        encode_place_dict(&self.places)
    }
}

#[cfg(test)]
#[path = "geocode.test.rs"]
mod test;
