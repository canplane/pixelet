// pxl-spec/search.spec.rs
//
// GENERATED from spec/spec/search.json (xpute-gen data.rs)
//
// What the search endpoint (route.json `SEARCH_URL`) answers, by number: a
// result's kind, which says what its `category` numbers. A place's is its
// category as a place tile carries it (bake/place_category.json, counted from
// 1, 0 for none); a division's is its subtype (division.json), as each part
// of a result's `context` gives its own.

pub mod search_kind {
    pub const PLACE: u8 = 0;
    pub const DIVISION: u8 = 1;
}
