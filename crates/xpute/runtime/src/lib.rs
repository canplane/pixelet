// xpute-runtime/lib.rs

//! The guest's side of the runtime, over `xpute-core`. Using it means running
//! its way: a turn on a doorbell, a quota from the frame, I/O asked of the host.

pub mod abi;
pub mod clock;
pub mod global;
pub mod ipc;
pub mod mem;
pub mod sched;
