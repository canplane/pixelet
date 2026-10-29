// xpute-kit/alloc.rs

//! A buddy of pages and a slab of size classes over it, on a range the
//! caller names through `Range`.

/// The bits a range's size and the exponents the buddy keeps fit in: a
/// guest's memory is 32-bit whatever the machine the allocator is built for,
/// though on a native build the range's base is a full pointer.
pub const ADDR_BITS: usize = 32;

pub mod buddy_tree;
pub mod slab;
