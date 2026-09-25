// pxl-spec/lib.rs

//! pixelet's spec, in Rust: what every side follows and nothing decides
//! again — the space and its coordinates, the cell catalogs, the bake's
//! formats and the shipped assets' tables. The server and the client read the
//! same one, and there is no second implementation of it: the shell reads a
//! page through the kernel rather than decoding one. What the shell and the
//! kernel say to each other is the kernel's (pxl-kernel's `abi`).

pub mod asset;
pub mod bake;
pub mod cell;
pub mod place;
#[path = "route.spec.rs"]
pub mod route;
pub mod space;
