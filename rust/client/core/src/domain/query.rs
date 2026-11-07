// pxl-core/domain/query.rs

//! Queries over what is resident, all through the world's one query, what
//! is in a node (`World::node`): the asset in a cell at the zoom it is asked
//! (`World::asset`), and the first cell a ray meets (`first_hit`), which an
//! eye casts and a walker casts straight up and down its cell (`support.rs`).
//! A query never fetches or holds anything; it returns an exact answer, a
//! coarser one with the tile it lacked, or nothing and that tile.
//!
//! A cell is what the cast draws, by one rule, so picks and column surface stand
//! on what is on screen. A node coarser than asked is not what is drawn
//! there: it is asked only whether anything under it fills, at any zoom.
//!
//! The query is the only reader of a layer (ARCHITECTURE.md, "The query, the
//! only seam"): the layers' reads (`Query`, `TerrainQuery`, `ObjectQuery`,
//! `CpuClipmap`, the ways' pool) are named here and in the layers alone, and
//! none is re-exported (pxl-tool check_rules.rs `LAYER_READERS`). A public
//! `World` function answers from the composed world; one layer's answer
//! alone (`surface_of`) stays inside the query.

pub mod changes;
pub mod decor;
mod entry;
mod limit;
mod node;
mod place;
mod ray;
mod scatter;
mod support;
mod surface;
mod world;

pub use ray::{at_zoom, by_dist, cell_zoom_at, finest, zoom_range, Hit, HitKind, Ray};
pub use support::{over, under, walks};
pub use world::{ColumnWalk, World};

#[cfg(test)]
#[path = "query.test.rs"]
pub(crate) mod test;
