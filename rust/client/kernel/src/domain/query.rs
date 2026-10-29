// pxl-kernel/domain/query.rs

//! Queries over what is resident, all through the world's one query, what
//! is in a node (`World::node`, doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md):
//! the asset in a cell at the zoom it is asked (`World::asset`), and the first cell a
//! ray meets (`first_hit`), which an eye casts and a walker casts straight
//! up and down its cell (`support.rs`). A query never fetches or holds
//! anything; it returns an exact answer, a coarser one with the tile it
//! lacked, or nothing and that tile.
//!
//! A cell is what the cast draws, by one rule, so picks and footing stand
//! on what is on screen. A node coarser than asked is not what is drawn
//! there: it is asked only whether anything under it fills, at any zoom.

pub mod changed;
pub mod decor;
mod limit;
pub mod model;
mod node;
mod place;
mod placed;
mod ray;
mod scatter;
mod support;
mod surface;
mod world;

pub use ray::{asked, at_zoom, by_dist, cell_at, held, Hit, Ray, Struck};
pub use support::{over, under, walks};
pub use world::{Walked, World};

#[cfg(test)]
#[path = "query.test.rs"]
mod test;
