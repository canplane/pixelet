// pxl-core/sys.rs

//! The lowest layer of the core, which the domain stands on: its I/O, the
//! mechanics of a turn, the sections of its memory, the files it loads, what
//! it was booted with, the machine it asks of its runtime and the device
//! transport's decoder. Nothing here uses the domain, the renderer or the
//! profiler (pxl-tool rule.rs).

pub(crate) mod boot;
pub(crate) mod io;
pub(crate) mod load;
#[path = "sys/machine.spec.rs"]
pub mod machine;
pub(crate) mod sched;
pub(crate) mod section;
#[cfg(not(target_arch = "wasm32"))]
mod unpack;

/// Serializes the native tests that touch a `Global`. The test harness runs
/// tests on several threads, and a `Global` assumes one.
#[cfg(test)]
pub(crate) fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
