// pxl-geo-bake/geocode.rs

pub mod provider;

use std::collections::HashMap;

use xpute_core::status::error::MarshalError;

use pxl_spec::bake::division_tile::{encode_division_dict, Division, NONE};

/// A dictionary division as it is read: its id, its parent's, and itself.
pub type Entry = (String, Option<String>, Division);

/// Read whole before any group is written: a division is its position in
/// this list, so every reader must see the same order.
pub struct DivisionDict {
    pub divisions: Vec<Division>,
    at: HashMap<String, u32>,
}

impl DivisionDict {
    /// Each division's parent numbered by its place here; one the rows lack is none.
    pub fn of(rows: Vec<Entry>) -> DivisionDict {
        let at: HashMap<String, u32> = rows.iter().enumerate().map(|(i, (id, _, _))| (id.clone(), i as u32)).collect();
        let divisions = rows
            .into_iter()
            .map(|(_, parent, d)| Division {
                parent: parent.and_then(|p| at.get(&p).copied()).unwrap_or(NONE),
                ..d
            })
            .collect();
        DivisionDict { divisions, at }
    }

    pub fn number(&self, id: &str) -> Option<u32> {
        self.at.get(id).copied()
    }

    pub fn packet(&self) -> Result<Vec<u8>, MarshalError> {
        encode_division_dict(&self.divisions)
    }
}

#[cfg(test)]
#[path = "geocode.test.rs"]
mod test;
