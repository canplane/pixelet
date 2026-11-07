// xpute-core/lib.rs

//! The core's side of the runtime, over `xpute-kit`. Using it means running
//! its way: a turn on a doorbell, a deadline from the frame, I/O asked of the runtime.

pub mod abi;
pub mod call;
pub mod clock;
pub mod entry;
pub mod global;
pub mod ipc;
pub mod machine;
pub mod mem;
pub mod sched;
pub mod stop;
pub mod wasm;
