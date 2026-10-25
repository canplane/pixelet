// pxl-spec/division.rs

//! A place's name as the divisions it lies in, for both the live geocoder
//! and the bake: Overture's schema, a division its subtype, names and
//! parent, read as the source holds it. A name is a division and its
//! ancestors, broad to narrow; the client picks the depth, the order and the
//! language. A provider maps its own areas onto these subtypes in its adapter.

use std::collections::BTreeMap;

use crate::division_spec::DIVISION_SUBTYPE;

/// Overture's subtypes, broadest first: spec/spec/division.json's numbers.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Subtype {
    Country = DIVISION_SUBTYPE::COUNTRY,
    Dependency = DIVISION_SUBTYPE::DEPENDENCY,
    Macroregion = DIVISION_SUBTYPE::MACROREGION,
    Region = DIVISION_SUBTYPE::REGION,
    Macrocounty = DIVISION_SUBTYPE::MACROCOUNTY,
    County = DIVISION_SUBTYPE::COUNTY,
    Localadmin = DIVISION_SUBTYPE::LOCALADMIN,
    Locality = DIVISION_SUBTYPE::LOCALITY,
    Borough = DIVISION_SUBTYPE::BOROUGH,
    Macrohood = DIVISION_SUBTYPE::MACROHOOD,
    Neighborhood = DIVISION_SUBTYPE::NEIGHBORHOOD,
    Microhood = DIVISION_SUBTYPE::MICROHOOD,
}

impl Subtype {
    pub const ALL: [Subtype; 12] = [
        Subtype::Country,
        Subtype::Dependency,
        Subtype::Macroregion,
        Subtype::Region,
        Subtype::Macrocounty,
        Subtype::County,
        Subtype::Localadmin,
        Subtype::Locality,
        Subtype::Borough,
        Subtype::Macrohood,
        Subtype::Neighborhood,
        Subtype::Microhood,
    ];

    /// None for a number no subtype has.
    pub fn of(value: u8) -> Option<Subtype> {
        Subtype::ALL.into_iter().find(|s| *s as u8 == value)
    }

    /// Overture's word for it.
    pub fn name(self) -> &'static str {
        match self {
            Subtype::Country => "country",
            Subtype::Dependency => "dependency",
            Subtype::Macroregion => "macroregion",
            Subtype::Region => "region",
            Subtype::Macrocounty => "macrocounty",
            Subtype::County => "county",
            Subtype::Localadmin => "localadmin",
            Subtype::Locality => "locality",
            Subtype::Borough => "borough",
            Subtype::Macrohood => "macrohood",
            Subtype::Neighborhood => "neighborhood",
            Subtype::Microhood => "microhood",
        }
    }

    /// None for a word Overture has not given a subtype.
    pub fn named(word: &str) -> Option<Subtype> {
        Subtype::ALL.into_iter().find(|s| s.name() == word)
    }
}

/// One division of a name.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct DivisionPart {
    pub subtype: Subtype,
    /// The provider's own name, shown when `names` lacks the reader's language.
    pub name: String,
    /// By language tag; empty rather than absent.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "BTreeMap::is_empty")
    )]
    pub names: BTreeMap<String, String>,
    /// The division nearest the point rather than one containing it.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "std::ops::Not::not")
    )]
    pub approx: bool,
}
