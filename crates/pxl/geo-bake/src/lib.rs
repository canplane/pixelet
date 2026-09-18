//! The offline half of the geo data: what turns Overture and the elevation
//! raster into the static pages the bucket serves, and the tools that run it
//! over a coverage.
//!
//! It is `pxl-geo`'s counterpart rather than its dependency — the live server
//! reads three Divisions/Places tables and this reads the whole geo DB, they
//! run on different machines, and nothing passes between them but the spec.
//! See issue/20260918-the-server-in-rust-and-the-line-already-drawn.md.
//!
//! The tree still mirrors apps/api/src/bake, the TypeScript this was ported
//! from, which was kept beside it until the two had been compared page by page
//! and then deleted. The shape is worth keeping anyway: it is the one the
//! pipeline's own stages fall into, and a reader following the old file names
//! through git history lands in the right module.

pub mod geocode;
pub mod overture;
pub mod poi;
pub mod terrain;
pub mod util;
pub mod walk;
pub mod wkb;
