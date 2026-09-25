// pxl-spec/place.rs

//! What a place is called, and how far down the answer goes.
//!
//! Both servers answer with this: the live one gives a search result the
//! context that tells 수내동 in 분당구 from any other, and the bake writes
//! the label a page reads as. The client joins the parts into a line and
//! picks a language, because broad-to-narrow is a convention of the reader's
//! language rather than a property of the place — Korean, Japanese and
//! Chinese go broad-to-narrow and most others reverse it, the same way dates
//! do.
//!
//! A part carries a **rank**: where it sits on one ladder, from a country
//! down to a building. That is what lets a reader ask for the depth a zoom
//! wants — a map drawn from four hundred kilometers up names a province and
//! stops, and one at street level does not
//! (issue/20260923-a-place-name-is-a-rank-and-an-id.md). Nominatim's reverse
//! endpoint takes a zoom for the same reason and can, because every row of
//! its address table carries an `address_rank`; Google's response carries
//! `types` and Naver's carries `region.area1..4`. Without a rank the parts
//! are an ordered list nobody outside the provider can cut.
//!
//! **The ladder is ours.** A provider maps its own vocabulary onto it —
//! Overture's `region`, `county` and its four sub-city subtypes
//! (`pxl-geo-spec`), another's whatever it has — and nothing of that
//! vocabulary reaches this file. That is the line the two servers each drew
//! and each crossed, by passing a provider's own name map through as an
//! opaque blob.

use std::collections::BTreeMap;

/// Where a part sits, broadest first. The numbers are the identity — a page
/// stores one — so a rung is appended rather than inserted, and the ladder
/// is spaced so that a provider with a level between two of these has a rung
/// to map onto rather than a choice of two wrong ones.
///
/// Korea uses five of them: a 도 or 특별시 is a region, a 시/군/구 a county,
/// a 동 or 리 a neighborhood. The rest are here because the ladder is not
/// Korea's.
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
    /// Every rung, broadest first.
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

    /// The rung a stored number names, or none for a number no rung has.
    pub fn of(value: u8) -> Option<PlaceRank> {
        PlaceRank::ALL.into_iter().find(|r| *r as u8 == value)
    }

    /// The deepest rung a map at Web Mercator zoom `z` names, which is
    /// Nominatim's own table read onto this ladder: a country to z4, a state
    /// to z7, a county to z9, a city to z11, a town or village to z13, a
    /// neighborhood or settlement to z15, streets to z17, a building past
    /// that. The steps are what the convention is; nothing here was derived.
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
        // A zoom that is not a number is the whole ladder rather than none of
        // it: a label is better wrong at the bottom than missing.
        LADDER.iter().find(|(top, _)| z <= *top).map_or(PlaceRank::Building, |(_, rank)| *rank)
    }
}

/// One part of a place's name, with where it sits.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct PlacePart {
    pub rank: PlaceRank,
    /// What the provider calls it, in whatever language its rows are in:
    /// what to show when `names` has nothing for the reader's language.
    pub name: String,
    /// The same name by language tag, where the provider has one. Empty
    /// rather than absent, so a reader takes the same path either way.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "BTreeMap::is_empty"))]
    pub names: BTreeMap<String, String>,
    /// Set where the part came from a place near the query rather than one
    /// containing it: the label is then an approximation, and a reader may
    /// want to say so.
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

    /// The name a reader asking in `lang` sees.
    pub fn in_language(&self, lang: &str) -> &str {
        self.names.get(lang).map(String::as_str).unwrap_or(&self.name)
    }
}

/// A place named broad to narrow.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub parts: Vec<PlacePart>,
}

impl Place {
    /// The parts down to `deepest` and no further: what a reader at a zoom
    /// asks for. The parts are already broad to narrow, so this is a cut.
    pub fn to(&self, deepest: PlaceRank) -> impl Iterator<Item = &PlacePart> {
        self.parts.iter().filter(move |p| p.rank <= deepest)
    }
}
