// xpute-kit/alloc.rs

//! A buddy of pages and a slab of size classes over it, on a range the
//! caller names through `Range`.

pub mod buddy_tree;
pub mod slab;
