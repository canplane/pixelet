// pxl-asset-bake/lib.rs

//! The shipped asset tree's own tooling: what turns `asset/` into the objects
//! the bucket holds and the tables the catalogs read.
//!
//! Not the bake. Nothing here touches db-geo or a page — the input is files in
//! the repository and the output is their content-addressed copies, one
//! composed texture sheet, and the two generated tables that name them. It is
//! a crate rather than a module of pxl-spec because it *writes* pxl-spec's
//! `asset::ingested`, and a generator that lives inside what it generates is
//! a circle.

pub mod brand;
pub mod glb;
pub mod pack;
pub mod sheet;
pub mod table;
