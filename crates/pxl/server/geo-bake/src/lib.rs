// pxl-geo-bake/lib.rs

//! The offline half of the geo data: what turns Overture and the elevation
//! raster into the static pages the bucket serves, and the tools that run it
//! over a coverage.
//!
//! It is `pxl-geo`'s counterpart rather than its dependency — the live server
//! reads three Divisions/Places tables and this reads the whole geo DB, they
//! run on different machines, and nothing passes between them but the spec.
//! See issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.
//!
//! One module per stage of the pipeline: `terrain` bakes a page from the
//! geo DB, `poi` and `geocode` each fetch what one page holds, and `page` is
//! the walk over a coverage that the last two share. What a bake does is a
//! sequence, and the tree says which step a file belongs to rather than which
//! type it holds. The command lines that drive them are `src/bin/`, outside
//! the library — each links it by name like any other consumer.

pub mod geocode;
pub mod page;
pub mod poi;
pub mod terrain;
pub mod util;
pub mod wkb;
