// pxl-geo-spec/overture/division.rs

use pxl_spec::place::PlaceRank;

/// The three sub-city subtypes share a rung because Overture does not
/// guarantee they nest.
pub fn rank_of(subtype: &str) -> Option<PlaceRank> {
    Some(match subtype {
        "region" => PlaceRank::Region,
        "county" => PlaceRank::County,
        "macrohood" => PlaceRank::District,
        "locality" | "neighborhood" | "microhood" => PlaceRank::Neighborhood,
        _ => return None,
    })
}

pub const BROAD_SUBTYPE_ORDER: [&str; 2] = ["region", "county"];

/// A formal 동 (`locality`) wins over an informal area at the same point.
pub const FINE_SUBTYPE_PRIORITY: [&str; 4] = ["locality", "neighborhood", "microhood", "macrohood"];

pub fn broad_rank(subtype: &str) -> Option<usize> {
    BROAD_SUBTYPE_ORDER.iter().position(|s| *s == subtype)
}

#[cfg(test)]
#[path = "division.test.rs"]
mod test;
