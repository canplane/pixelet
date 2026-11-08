// xpute-core/lib.rs

//! The library OS a core is built on, over `xpute-kit`. A core built on it
//! runs in turns that its runtime starts when a doorbell rings, each turn
//! with a deadline the runtime gives, and it asks the runtime for all its
//! I/O.

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
