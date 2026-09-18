// pxl-overture/division.rs

//! Overture's Divisions subtypes, and the order each ladder is read in.
//!
//! Both servers resolve a point against the same table and have to agree about
//! what the fine level of a place is called — a search result's context and the
//! current-location label naming different things for the same point reads as a
//! bug wherever they are shown together. These were the same two lists in two
//! files before.
//!
//! What is *not* here is the shape either server answers in. `GeocodePart` is a
//! wire type of ours: swap the provider and it stays while these lists change,
//! so it belongs to whoever is answering rather than to what was asked.

/// region (도/특별시) → county (시/구), the canonical broad-to-narrow order.
/// Sorted strictly by subtype rather than by name pattern or polygon size:
/// Korea's administrative depth is not uniform — Seoul has three real levels
/// under `region`, and in a split city like Seongnam `county` lands on the city
/// rather than on a district — so a reader gets whatever is actually the finest
/// broad-level data Overture has for a point rather than a fixed real-world
/// meaning assumed per subtype.
pub const BROAD_SUBTYPE_ORDER: [&str; 2] = ["region", "county"];

/// Overture's four sub-city subtypes, most-specific-for-Korea first: a formal
/// 동 (`locality`) is what most reverse-geocode consumers actually want, so it
/// wins over an informal or commercial area sharing the same point even when
/// both have a polygon there. Only the first match is used — unlike
/// region ⊃ county, Overture does not guarantee these four nest cleanly, since
/// a `microhood` can be a business district overlapping a `locality` without
/// being contained by any real hierarchy.
pub const FINE_SUBTYPE_PRIORITY: [&str; 4] = ["locality", "neighborhood", "microhood", "macrohood"];

/// The rank of a broad subtype, or none when it is not one.
pub fn broad_rank(subtype: &str) -> Option<usize> {
    BROAD_SUBTYPE_ORDER.iter().position(|s| *s == subtype)
}
