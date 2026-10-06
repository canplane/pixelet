// pxl-kernel/lib.rs

//! pixelet's kernel: the world's state and what runs over it. Nothing here
//! knows of WebAssembly, a browser or threads (pxl-wasm does), so the same
//! code runs under `cargo test` and in the client's module.

pub mod abi;
pub mod control;
pub mod door;
pub mod host;
pub mod io;
pub mod kernel;
pub mod layer;
pub mod mem;
pub mod param;
pub mod scene;
pub mod sched;
pub mod shell;
pub mod text;
pub mod world;

/// Native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
#[path = "allocations.test.rs"]
pub mod allocations;
