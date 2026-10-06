// pxl-spec/space/sparse_tree.rs

//! An octree kept in two flat lanes rather than in nodes: the memory the
//! lanes are cut from, and the tree read and written over it. A tile's cells
//! are mostly empty, so what is stored is what differs from the default.

pub mod mem;
pub mod tree;
