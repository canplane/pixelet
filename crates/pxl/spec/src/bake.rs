// pxl-spec/bake.rs

//! What a baked page is, as both ends read it: the formats a terrain or POI
//! page is written in and decoded from, the cell catalog a ground channel is
//! numbered by, which of a cell's four channels a viewer sees, how those four
//! are packed into one word, and the
//! version the client asks for. One implementation, so a page the bake wrote
//! and a page the kernel reads cannot drift.

pub mod constant;
pub mod fold;
pub mod format;
pub mod ground;
pub mod model;
pub mod pack;
pub mod place_page;
#[path = "bake/poi_badge.spec.rs"]
pub mod poi_badge;
pub mod poi_page;
#[path = "bake/published.spec.rs"]
pub mod published;
pub mod quadkey;
