// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it. Nothing here
//! knows of WebAssembly, a browser or threads (pxl-wasm does), so the same
//! code runs under `cargo test` and in the client's module.

pub mod abi;
pub mod control;
pub mod door;
pub mod global;
pub mod host;
pub mod io;
pub mod kernel;
pub mod layer;
pub mod mem;
pub mod param;
pub mod sched;
pub mod shell;
pub mod text;
pub mod world;

#[cfg(test)]
#[path = "allocations.test.rs"]
pub mod allocations;
