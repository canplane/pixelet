// pxl-geo-spec/overture/division.rs

//! Where Overture's Divisions subtypes sit on the ladder both servers answer
//! on (`pxl-spec place.rs`).
//!
//! Both resolve a point against the same table and have to agree about what
//! the fine level of a place is called — a search result's context and the
//! current-location label naming different things for the same point reads as
//! a bug wherever they are shown together. These were the same two lists in
//! two files before.
//!
//! What is *not* here is the shape either server answers in: that is
//! `pxl-spec`'s, and it survives a change of provider while this file does
//! not. This file is the whole of what Overture's vocabulary means to us.

use pxl_spec::place::PlaceRank;

/// The rung a subtype sits on, or none for a subtype the answer has no room
/// for. Korea's administrative depth is not uniform — Seoul has three real
/// levels under `region`, and in a split city like Seongnam `county` lands on
/// the city rather than on a district — so a reader gets whatever Overture
/// actually has for a point rather than a real-world meaning assumed per
/// subtype.
///
/// The three sub-city subtypes share a rung because Overture does not
/// guarantee they nest: a `microhood` can be a business district overlapping
/// a `locality` without being contained by any real hierarchy. Which of them
/// wins where several contain one point is `FINE_SUBTYPE_PRIORITY`.
pub fn rank_of(subtype: &str) -> Option<PlaceRank> {
    Some(match subtype {
        "region" => PlaceRank::Region,
        "county" => PlaceRank::County,
        "macrohood" => PlaceRank::District,
        "locality" | "neighborhood" | "microhood" => PlaceRank::Neighborhood,
        _ => return None,
    })
}

/// The subtypes that nest, broadest first: every one containing a point is a
/// part of the answer.
pub const BROAD_SUBTYPE_ORDER: [&str; 2] = ["region", "county"];

/// Overture's four sub-city subtypes, most-specific-for-Korea first: a formal
/// 동 (`locality`) is what most reverse-geocode consumers actually want, so it
/// wins over an informal or commercial area sharing the same point even when
/// both have a polygon there. Only the first match is used.
pub const FINE_SUBTYPE_PRIORITY: [&str; 4] = ["locality", "neighborhood", "microhood", "macrohood"];

/// The rank of a broad subtype, or none when it is not one.
pub fn broad_rank(subtype: &str) -> Option<usize> {
    BROAD_SUBTYPE_ORDER.iter().position(|s| *s == subtype)
}

#[cfg(test)]
#[path = "division.test.rs"]
mod test;
