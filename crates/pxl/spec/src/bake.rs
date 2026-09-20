// pxl-spec/bake.rs

//! What a baked page is, as both ends read it: the formats a terrain or POI
//! page is written in and decoded from, the cell catalog a ground channel is
//! numbered by, which of a cell's four channels a viewer sees, and the
//! version the client asks for. One implementation, so a page the bake wrote
//! and a page the kernel reads cannot drift.

#[path = "bake/cell.spec.rs"]
pub mod cell_spec;
pub mod constant;
pub mod format;
pub mod ground;
pub mod model;
#[path = "bake/published.spec.rs"]
pub mod published;
