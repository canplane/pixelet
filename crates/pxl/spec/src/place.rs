// pxl-spec/place.rs

//! A place's name as ranked parts, for both the live geocoder and the bake.
//! The rank lets a reader cut the name at the depth a zoom wants; the client
//! picks the order and language. Providers map their own vocabulary onto
//! this ladder in their adapters.

use std::collections::BTreeMap;

/// Where a part sits, broadest first. The numbers are stored, so a rung is
/// appended, never inserted; the gaps leave room for a provider's in-between
/// levels.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PlaceRank {
    Country = 0,
    Region = 10,
    County = 20,
    City = 30,
    District = 40,
    Neighborhood = 50,
    Street = 60,
    Building = 70,
}

impl PlaceRank {
    pub const ALL: [PlaceRank; 8] = [
        PlaceRank::Country,
        PlaceRank::Region,
        PlaceRank::County,
        PlaceRank::City,
        PlaceRank::District,
        PlaceRank::Neighborhood,
        PlaceRank::Street,
        PlaceRank::Building,
    ];

    /// None for a number no rung has.
    pub fn of(value: u8) -> Option<PlaceRank> {
        PlaceRank::ALL.into_iter().find(|r| *r as u8 == value)
    }

    /// The deepest rung a map at Web Mercator zoom `z` names: Nominatim's
    /// zoom table read onto this ladder.
    pub fn at_zoom(z: f64) -> PlaceRank {
        const LADDER: [(f64, PlaceRank); 7] = [
            (4.0, PlaceRank::Country),
            (7.0, PlaceRank::Region),
            (9.0, PlaceRank::County),
            (11.0, PlaceRank::City),
            (13.0, PlaceRank::District),
            (15.0, PlaceRank::Neighborhood),
            (17.0, PlaceRank::Street),
        ];
        // A NaN zoom gets the whole ladder: a label is better wrong than missing.
        LADDER.iter().find(|(top, _)| z <= *top).map_or(PlaceRank::Building, |(_, rank)| *rank)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlacePart {
    pub rank: PlaceRank,
    /// The provider's own name, shown when `names` lacks the reader's language.
    pub name: String,
    /// By language tag; empty rather than absent.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "BTreeMap::is_empty"))]
    pub names: BTreeMap<String, String>,
    /// From a place near the query rather than one containing it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
    pub approx: bool,
}

impl PlacePart {
    pub fn new(rank: PlaceRank, name: String, names: BTreeMap<String, String>) -> PlacePart {
        PlacePart { rank, name, names, approx: false }
    }

    pub fn approx(mut self) -> PlacePart {
        self.approx = true;
        self
    }

    pub fn in_language(&self, lang: &str) -> &str {
        self.names.get(lang).map(String::as_str).unwrap_or(&self.name)
    }
}

/// Broad to narrow.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub parts: Vec<PlacePart>,
}

impl Place {
    /// The parts down to `deepest`.
    pub fn to(&self, deepest: PlaceRank) -> impl Iterator<Item = &PlacePart> {
        self.parts.iter().filter(move |p| p.rank <= deepest)
    }
}
