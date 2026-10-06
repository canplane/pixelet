// pxl-geo-bake/lib.rs

//! Turns Overture and the elevation raster into the static tiles the bucket
//! serves. `deck` solves heights, `terrain` bakes tiles, `poi` and `geocode`
//! fetch per tile, `tile` walks a coverage.

pub mod cli;
pub mod coverage;
pub mod deck;
pub mod elevation;
pub mod geocode;
pub mod poi;
pub mod pyramid;
pub mod segment;
pub mod terrain;
pub mod tile;
pub mod util;
pub mod wkb;

#[cfg(feature = "serve")]
pub mod demand;
