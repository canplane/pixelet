// pxl-kernel/query.rs

//! Queries over what is resident, all through the world's one query, what
//! is in a node (`World::node`, doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md):
//! a block at the zoom it is asked (`World::block`), and the first block a
//! ray meets (`first_hit`), which an eye casts and a walker casts straight
//! up and down its cell (`support.rs`). A query never fetches or holds
//! anything; it returns an exact answer, a coarser one with the tile it
//! lacked, or nothing and that tile.
//!
//! A block is what the cast draws, by one rule, so picks and footing stand
//! on what is on screen. A node coarser than asked is not what is drawn
//! there: it is asked only whether anything under it fills, at any zoom.

pub mod changed;
mod decor;
mod limit;
mod node;
mod place;
mod placed;
mod ray;
mod scatter;
mod support;
mod surface;
mod world;

pub use ray::{asked, at_zoom, by_distance, Hit, Ray, Struck};
pub use support::{cell_at, over, under, walks};
pub use world::World;

#[cfg(test)]
#[path = "query.test.rs"]
mod test;
