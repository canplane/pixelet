//! The offline half of the geo data: what turns Overture and the elevation
//! raster into the static pages the bucket serves, and the tools that run it
//! over a coverage.
//!
//! It is `pxl-geo`'s counterpart rather than its dependency — the live server
//! reads three Divisions/Places tables and this reads the whole geo DB, they
//! run on different machines, and nothing passes between them but the spec.
//! See issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.
//!
//! One module per stage of the pipeline, and `tool/` under each for the command
//! line that drives it: what a bake does is a sequence, and the tree says which
//! step a file belongs to rather than which type it holds.

pub mod geocode;
pub mod overture;
pub mod poi;
pub mod terrain;
pub mod util;
pub mod walk;
pub mod wkb;
