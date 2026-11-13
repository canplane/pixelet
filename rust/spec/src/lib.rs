// pxl-spec/lib.rs

//! pixelet's spec in Rust: space and coordinates, the asset catalog and its file
//! tables, bake formats, shared by server and client.

pub mod asset;
pub mod bake;
pub mod brick;
pub mod color;
pub mod directory;
pub mod division;
#[path = "division.spec.rs"]
mod division_spec;
pub mod ground;
pub mod object;
pub mod param;
pub mod procedural;
#[path = "route.spec.rs"]
pub mod route;
#[path = "search.spec.rs"]
pub mod search;
pub mod share;
pub mod space;
pub mod value;
pub mod way;
