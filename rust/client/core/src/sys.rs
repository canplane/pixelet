// pxl-core/sys.rs

//! The lowest layer of the core, which the domain stands on: its I/O, the
//! mechanics of a turn, the sections of its memory, the files it loads and
//! what it was booted with. Nothing here uses the domain, the renderer or the
//! profiler (pxl-tool check_rules.rs).

pub(crate) mod boot;
pub(crate) mod io;
pub(crate) mod load;
pub mod mem;
pub(crate) mod sched;

/// Serializes the native tests that touch a `Global`. The test harness runs
/// tests on several threads, and a `Global` assumes one.
#[cfg(test)]
pub(crate) fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
