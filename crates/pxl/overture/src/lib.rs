//! What Overture's data means here.
//!
//! Places and Divisions are read by both servers — `pxl-geo` answers forward
//! search out of them live, `pxl-geo-bake` bakes POI and reverse-geocode pages
//! from them offline — and neither depends on the other
//! (issue/20260918-the-server-in-rust-and-the-line-already-drawn.md). What they
//! share is not a query but a vocabulary: the same Overture category has to
//! resolve to the same one of ours whichever side asked, or a place's badge on
//! the map and its row in the search list draw different icons.
//!
//! So this holds the translation and the accessors over Overture's `properties`
//! JSON, and nothing else. It is not the Overture tables' data layer: each
//! server keeps its own statements, because what they ask for differs even
//! where the table does not.

pub mod category;
pub mod division;
pub mod properties;
