// xpute-runtime/lib.rs

//! The runtime, in Rust: what a guest and its host agree on, over
//! `xpute-core`'s kit.
//!
//! The rings and the change log a linear memory's two sides talk across, the
//! three levels of the scheduler that divide one turn, the clock and the
//! memory a browser does not give a module, and the one value per module that
//! running on a single thread makes sound.
//!
//! Using this means running the way it runs: a turn on a doorbell, a quota
//! measured from the frame, I/O granted by credits. That is the line between
//! it and `xpute-core`, which asks nothing of its caller.

pub mod abi;
pub mod clock;
pub mod global;
pub mod io;
pub mod ipc;
pub mod mem;
pub mod sched;
