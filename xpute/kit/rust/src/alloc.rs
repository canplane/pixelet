// xpute-kit/alloc.rs

//! This is a buddy allocator of pages, a slab allocator of size classes
//! that takes its pages from the buddy, and a heap that takes the slab under
//! a lock, as a process's global allocator. Each manages a range of memory
//! that the caller describes through the `Range` trait.

/// The width in bits that a range's size and the buddy's exponents fit in;
/// the range's base alone is a full pointer.
pub const ADDR_BITS: usize = 32;

pub mod buddy_tree;
pub mod heap;
pub mod slab;
