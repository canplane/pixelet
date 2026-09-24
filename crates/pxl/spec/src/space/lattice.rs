// pxl-spec/space/lattice.rs

//! Structures over an integer lattice: a continuous coordinate quantised to
//! a cell, and the cell's address hashed to a bucket. `hash` is the one
//! address hash they share; `spatial_hash` buckets points in 3D.

pub mod hash;
pub mod spatial_hash;
