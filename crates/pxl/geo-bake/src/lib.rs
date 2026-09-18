//! The offline half of the geo data: what turns Overture and the elevation
//! raster into the static pages the bucket serves, and the tools that run it
//! over a coverage.
//!
//! It is `pxl-geo`'s counterpart rather than its dependency — the live server
//! reads three Divisions/Places tables and this reads the whole geo DB, they
//! run on different machines, and nothing passes between them but the spec.
//! See issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.
//!
//! The tree mirrors apps/api/src/bake, whose TypeScript this is ported from
//! and which stays beside it: a page baked here has to come out byte for byte
//! what that produces, and the comparison needs both.

pub mod geocode;
pub mod overture;
pub mod poi;
pub mod terrain;
pub mod util;
pub mod walk;
pub mod wkb;
