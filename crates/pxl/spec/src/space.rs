// pxl-spec/space.rs

//! Where anything is, and in what: the fixed-point vector the world is
//! addressed by (pvf3), the levels a coordinate indexes into, the two map
//! projections a real location arrives in, and the structures over an integer
//! lattice a page's cells are kept in. Every side computes a position the
//! same way because it computes it here.

pub mod adapter;
pub mod indexer;
pub mod lattice;
pub mod pvf3;
pub mod sparse_tree;
pub mod v3;
pub mod xy;
