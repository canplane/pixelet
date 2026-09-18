//! The runtime, in Rust: what a guest and its host agree on, over
//! `xpute-core`'s kit.
//!
//! The rings and the change log a linear memory's two sides talk across, the
//! three levels of the scheduler that divide one turn, and what the host has
//! to give a module that a browser does not — a clock, a global that outlives
//! a call, and memory carved into ranges and a heap.
//!
//! Using this means running the way it runs: a turn on a doorbell, a quota
//! measured from the frame, I/O granted by credits. That is the line between
//! it and `xpute-core`, which asks nothing of its caller.

pub mod ipc;
pub mod sched;
pub mod sys;
