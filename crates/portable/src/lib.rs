// pxl-portable/lib.rs

//! gpu-portable: one source for the CPU and, through rust-gpu, WGSL.
//!
//! What pixelet's spec computes the same on the CPU and on the GPU, in
//! `core` alone: a zoom, a node's seed and the caves. pxl-spec takes it as
//! its own, and a shader crate depends on it as it is.

#![no_std]

pub mod cast;
pub mod caves;
#[path = "cube.spec.rs"]
pub mod cube;
pub mod ground;

/// Every size is a zoom (pxl-spec space/coord/zoom.rs).
pub type Zoom = u32;

/// A unit's: 1u is a z24 cell's side, X going once round the world in 2^24
/// of them (pxl-spec space/coord/zoom.rs).
pub const UNIT_ZOOM: Zoom = 24;

/// The finest tile zoom (pxl-spec space/layer/tile.rs).
pub const LEAF_ZOOM: Zoom = 15;

/// A tile is 2^9 cells a side, so a leaf tile's cells are at this zoom: the
/// ground's finest cell (pxl-spec space/layer/lattice.rs `GROUND`).
pub const LEAF_CELL: Zoom = LEAF_ZOOM + 9;
