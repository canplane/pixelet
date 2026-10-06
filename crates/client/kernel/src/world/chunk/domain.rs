// pxl-kernel/world/chunk/domain.rs

//! The frontier's goal: the view's ground cut into pyramid chunks, the same
//! for every layer. Residency never reads it.
//!
//! One walk gives every quality step's cut: a chunk is a piece over an
//! interval of steps. It runs a slice a turn on the observation it began
//! with, and is published whole.
//!
//! The walk starts from the 3D roots within reach (`SHARD_ZOOM`), split while
//! their cells are coarser than any the top's reach draws (`Lattice::grid_zoom`).
//! Every chunk it then starts from that holds anything is a piece, all around
//! the eye, so ground turned to shows a piece and never a hole. A chunk
//! splits only where some patch's footprint touches it and its projected
//! error crosses a cell (`Lens::error` against `cell_angle`). A chunk splits
//! only within `split_reach`, which bounds the goal whatever the screen. A
//! coarser quality splits a subset of what a finer one does.

mod job;
mod walk;

use pxl_spec::space::camera::grid::Tau;

use crate::world::chunk::mesh_slot::POOLS;
use crate::world::chunk::Chunk;

pub use job::{advance, current, objects_written, put, start, take_work, walking};
pub use walk::Domain;

/// Mesh slots per pool.
pub trait Price {
    fn piece(&self, c: Chunk, near: f64, tau: Tau) -> [u32; POOLS];
    fn split(&self, c: Chunk) -> [u32; POOLS];
    /// stop counting a step needing `n`
    fn over(&self, n: &[u32; POOLS]) -> bool;
}

#[cfg(test)]
#[path = "domain.test.rs"]
mod test;
