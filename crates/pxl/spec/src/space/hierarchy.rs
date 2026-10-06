// pxl-spec/space/hierarchy.rs

//! One hierarchy over every zoom, in any number of axes: a node's parent,
//! children and slot, its one key, and a sparse tree of them.

pub mod morton;
pub mod sparse_tree;
pub mod tree;
