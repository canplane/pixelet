// xpute-runtime/sys.rs

//! What the host has to give a module that a browser does not: a clock, a
//! value that outlives a call, and memory carved into ranges and a heap.

pub mod clock;
pub mod global;
pub mod mem;
