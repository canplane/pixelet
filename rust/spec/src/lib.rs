// pxl-spec/lib.rs

//! pixelet's spec in Rust: space and coordinates, the asset catalog and its file
//! tables, bake formats, shared by server and client with no second implementation.

pub mod asset;
pub mod bake;
pub mod dirs;
pub mod division;
#[path = "division.spec.rs"]
mod division_spec;
pub mod limit;
pub mod object;
pub mod param;
pub mod procedural;
#[path = "route.spec.rs"]
pub mod route;
pub mod space;
