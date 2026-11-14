// pxl-spec/division.rs

//! A place's name as the divisions it lies in, for both the live geocoder
//! and the bake: Overture's schema, a division its subtype, names and
//! parent, read as the source holds it. A name is a division and its
//! ancestors, broad to narrow; the client picks the depth, the order and the
//! language. A provider maps its own areas onto these subtypes in its adapter.

use crate::division_spec::{division_name, division_subtype};

/// The most bytes a division's name takes (division.json `DIVISION_NAME`).
pub const NAME_BYTES: usize = division_name::BYTES as usize;

/// Overture's subtypes, broadest first, numbered as spec/spec/division.json numbers them,
/// which is also how a subtype crosses as JSON.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Subtype {
    Country = division_subtype::COUNTRY,
    Dependency = division_subtype::DEPENDENCY,
    Macroregion = division_subtype::MACROREGION,
    Region = division_subtype::REGION,
    Macrocounty = division_subtype::MACROCOUNTY,
    County = division_subtype::COUNTY,
    Localadmin = division_subtype::LOCALADMIN,
    Locality = division_subtype::LOCALITY,
    Borough = division_subtype::BOROUGH,
    Macrohood = division_subtype::MACROHOOD,
    Neighborhood = division_subtype::NEIGHBORHOOD,
    Microhood = division_subtype::MICROHOOD,
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
    pub fn of(val: u8) -> Option<Subtype> {
        Subtype::ALL.into_iter().find(|s| *s as u8 == val)
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
    pub fn of_word(word: &str) -> Option<Subtype> {
        Subtype::ALL.into_iter().find(|s| s.name() == word)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Subtype {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(*self as u8)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Subtype {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let val = <u8 as serde::Deserialize>::deserialize(d)?;
        Subtype::of(val).ok_or_else(|| D::Error::custom("no such subtype"))
    }
}

/// One division of a name.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct DivisionPart {
    pub subtype: Subtype,
    /// The division's own name, `names.primary`, whatever the reader's language.
    pub name: String,
    /// The division nearest the point rather than one containing it.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "std::ops::Not::not")
    )]
    pub approx: bool,
}
