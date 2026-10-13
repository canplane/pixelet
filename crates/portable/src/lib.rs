// pxl-portable/lib.rs

//! gpu-portable: one source for the CPU and, through rust-gpu, WGSL.
//!
//! What pixelet's spec computes the same on the CPU and on the GPU, in
//! `core` alone: the zooms and the lattices of cells on them, a node's seed
//! and the caves, the ground and the service area it ends at, and the ray's
//! descent. pxl-spec takes it as its own, and a shader crate depends on it as
//! it is.

#![no_std]

pub mod area;
pub mod cast;
pub mod caves;
#[path = "cube.spec.rs"]
pub mod cube;
pub mod ground;
pub mod lattice;

/// Every size is a zoom (pxl-spec space/coord/zoom.rs).
pub type Zoom = u32;

/// A unit's: 1u is a z24 cell's side, X going once round the world in 2^24
/// of them (pxl-spec space/coord/zoom.rs).
pub const UNIT_ZOOM: Zoom = 24;

/// The finest tile zoom (pxl-spec space/layer/tile.rs).
pub const LEAF_ZOOM: Zoom = 15;

/// A tile is 2^9 cells a side, so a leaf tile's cells are at this zoom: the
/// ground's finest cell (`lattice::GROUND`).
pub const LEAF_CELL: Zoom = LEAF_ZOOM + 9;

/// A Q3 position's fraction bits: a unit is 256 steps (pxl-spec
/// space/coord/q3.rs).
pub const FRAC_W: u32 = 8;

/// The finest zoom a position names: a step of a Q3.
pub const FINEST_ZOOM: Zoom = UNIT_ZOOM + FRAC_W;

/// The world's height in units, as bits: Z spans 2^16 of them about sea
/// level (pxl-spec space/coord/world.rs `EXTENT`).
pub const HEIGHT_BITS: u32 = 16;

/// The coarsest cube: a tile here is as wide as the world is tall, so the
/// pyramid is alike on every axis from here finer, and 3D begins. A tile of
/// it with the world's whole height over it is a **shard**, the unit the
/// world's data is ingested, baked and kept in.
pub const SHARD_ZOOM: Zoom = UNIT_ZOOM - HEIGHT_BITS;

/// A chunk, what is built and drawn as one, is 2^CHUNK_STEPS of the camera
/// grid's cells a side (pxl-spec space/camera/grid.rs).
pub const CHUNK_STEPS: Zoom = 5;

/// How many `inner` cells stand across one `outer`, which is no finer.
pub const fn count(inner: Zoom, outer: Zoom) -> u32 {
    if inner < outer {
        panic!("an outer zoom finer than the inner")
    }
    1 << (inner - outer)
}
