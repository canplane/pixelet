// pxl-spec/search.spec.rs
//
// GENERATED from spec/spec/search.json
//
// What the search endpoint (route.json `SEARCH_URL`) answers, by number: a
// result's kind, which says what its `category` numbers. A place's is its
// category as a place tile carries it (bake/place_category.json, counted from
// 1, 0 for none); a division's is its subtype (division.json), as each part
// of a result's `context` gives its own.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    dead_code,
    reason = "generated from the spec"
)]

pub mod SEARCH_KIND {
    pub const PLACE: u8 = 0;
    pub const DIVISION: u8 = 1;
}
