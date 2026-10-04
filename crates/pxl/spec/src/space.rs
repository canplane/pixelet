// pxl-spec/space.rs

//! Positions and their structures: the fixed-point vector, the space
//! hierarchy, map projections and lattice structures.

pub mod adapter;
#[path = "space/earth.spec.rs"]
pub mod earth;
pub mod hierarchy;
pub mod indexer;
pub mod lattice;
pub mod lod;
pub mod mat4;
pub mod observation;
pub mod pvf3;
pub mod pyramid;
pub mod sparse_tree;
pub mod v3;
pub mod xy;
