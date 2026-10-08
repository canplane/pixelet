// pxl-spec/lib.rs

//! pixelet's spec in Rust: space and coordinates, the block catalog, bake formats
//! and asset tables, shared by server and client with no second implementation.

pub mod asset;
pub mod bake;
pub mod block;
pub mod division;
#[path = "division.spec.rs"]
mod division_spec;
pub mod procedural;
#[path = "route.spec.rs"]
pub mod route;
pub mod space;
