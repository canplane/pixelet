// xpute-runtime/mem.rs

//! The module's memory: where its ranges lie, and where a block comes from.
//!
//! Laying ranges end to end is a linker script's job and taking a block out
//! of the heap is a `malloc`'s, which is why they are two files rather than
//! one module doing both.

pub mod heap;
pub mod section;
