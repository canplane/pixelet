// xpute-core/alloc.rs

//! Allocators over a range the caller names: a buddy of pages, and a slab of
//! size classes over the buddy for everything smaller than a page. Neither
//! knows where the range is or what installs it — `Range` says, and a program
//! sets the pair as its global allocator or not.

pub mod buddy_tree;
pub mod slab;
