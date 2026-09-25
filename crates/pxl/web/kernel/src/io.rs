// pxl-kernel/io.rs

//! The kernel's I/O: what it asks the host to read, in what order, and what
//! becomes of what arrives.
//!
//! What is wanted is the producers'; where each want sits in one order is
//! `policy`'s; how many run at once, which of them, and what may be tried
//! again is `sched`'s, which is the only authority over them. What a body
//! decodes into is `fetch`'s.

pub mod demand;
pub mod fetch;
pub mod load;
pub mod policy;
pub mod sched;
