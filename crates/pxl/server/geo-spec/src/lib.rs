// pxl-geo-spec/lib.rs

//! What `pxl-geo` and `pxl-geo-bake` agree on about db-geo.
//!
//! The live server answers forward search out of those tables and the bake
//! writes POI and reverse-geocode pages from them, and neither depends on the
//! other (issue/20260918-the-server-in-rust-and-the-line-already-drawn.md).
//! What they share is not a query — each keeps its own statements, because
//! what they ask for differs even where the table does not — but the names and
//! the meanings underneath one.
//!
//! It holds nothing either of them *answers* with: a wire shape of ours is
//! `pxl-spec`'s, and survives a change of provider.

pub mod overture;
pub mod table;
