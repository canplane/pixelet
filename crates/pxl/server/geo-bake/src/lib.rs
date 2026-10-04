// pxl-geo-bake/lib.rs

//! Turns Overture and the elevation raster into the static pages the bucket
//! serves. `deck` solves heights, `terrain` bakes pages, `poi` and `geocode`
//! fetch per page, `page` walks a coverage.

pub mod cli;
pub mod geocode;
pub mod page;
pub mod poi;
pub mod pyramid;
pub mod terrain;
pub mod util;
pub mod wkb;

#[cfg(feature = "serve")]
pub mod demand;
