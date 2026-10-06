// pxl-spec/place.rs

//! A place's name as ranked parts, for both the live geocoder and the bake.
//! The rank lets a reader cut the name at the depth a zoom wants; the client
//! picks the depth, the order and the language. Providers map their own
//! vocabulary onto this ladder in their adapters.

use std::collections::BTreeMap;

use crate::place_spec::PLACE_RANK;

/// Where a part sits, broadest first: spec/spec/place.json's numbers.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PlaceRank {
    Country = PLACE_RANK::COUNTRY,
    Region = PLACE_RANK::REGION,
    County = PLACE_RANK::COUNTY,
    City = PLACE_RANK::CITY,
    District = PLACE_RANK::DISTRICT,
    Neighborhood = PLACE_RANK::NEIGHBORHOOD,
    Street = PLACE_RANK::STREET,
    Building = PLACE_RANK::BUILDING,
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
