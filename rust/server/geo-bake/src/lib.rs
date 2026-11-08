// pxl-geo-bake/lib.rs

//! Turns Overture and the elevation raster into the static tiles the bucket serves.
//! `separation` solves the heights of bridges and tunnels, `terrain` bakes the terrain tiles,
//! `place` reads the places for each tile, `division` makes the divisions' leaves, and `tile`
//! walks a coverage.

pub mod cli;
pub mod coverage;
pub mod division;
pub mod elevation;
pub mod place;
pub mod pyramid;
pub mod segment;
pub mod separation;
pub mod source;
pub mod terrain;
pub mod tile;
pub mod util;
pub mod wkb;

#[cfg(feature = "serve")]
pub mod demand;
