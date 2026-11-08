// xpute-kit/alloc.rs

//! This is a buddy allocator of pages, and a slab allocator of size classes
//! that takes its pages from the buddy. Both manage a range of memory that the
//! caller describes through the `Range` trait.

/// The width in bits that a range's size and the buddy's exponents fit in.
/// A core's memory is 32-bit on every machine the allocator is built for;
/// on a native build only the range's base is a full pointer.
pub const ADDR_BITS: usize = 32;

pub mod buddy_tree;
pub mod slab;
