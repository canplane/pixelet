// pxl-geo-bake/geocode.rs

//! Reverse geocoding: which places contain a page, as `pxl-spec`'s `Place`.
//!
//! The shape is not here and not the provider's. What is here is what every
//! provider has to do the same way: say where a part of a label came from,
//! and turn whatever it calls a name map into the one the answer carries.

pub mod provider;

use std::collections::{BTreeMap, HashMap};

use xpute_core::status::error::MarshalError;

use pxl_spec::bake::place_page::{encode_place_dict, Division};

/// Where one part of a page's label came from.
///
/// A polygon containing the page names a place the dictionary already has,
/// so the label carries its id and nothing else. A point *near* the page
/// names one nothing else has — the provider's points are a set of their own
/// — so the label carries the place itself and the group it is in keeps it
/// (pxl-spec bake/place_page.rs).
pub enum PartSource {
    Division(String),
    Near(Division),
}

/// Every place a polygon names, and the number a label refers to each by.
///
/// The number is a place in the dictionary, so the dictionary a group was
/// written against and the one a reader holds have to be the same list in the
/// same order. Both come from one read in the provider's own order, which is
/// why the whole thing is read before a single group is written.
pub struct PlaceDict {
    pub divisions: Vec<Division>,
    at: HashMap<String, u32>,
}

impl PlaceDict {
    /// From the provider's rows, each its id and the place it names.
    pub fn of(rows: Vec<(String, Division)>) -> PlaceDict {
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

/// A provider's name-by-language map, read into the answer's own. Overture
/// gives `names.common` as `{"en": …, "ko": …}`; anything that is not a map
/// of strings is nothing, because a name a reader cannot be shown is not a
/// name.
pub fn names_of(common: &Option<serde_json::Value>) -> BTreeMap<String, String> {
    let Some(serde_json::Value::Object(map)) = common else { return BTreeMap::new() };
    map.iter().filter_map(|(lang, name)| name.as_str().map(|n| (lang.clone(), n.to_string()))).collect()
}

#[cfg(test)]
#[path = "geocode.test.rs"]
mod test;
