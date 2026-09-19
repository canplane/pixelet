// pxl-kernel/global.rs

//! The kernel's name for the substrate's one-value-per-module device
//! (xpute global.rs), so a file wanting one asks the kernel rather than
//! reaching past it.
//!
//! What makes `Global` sound is xpute's promise and not this crate's: one
//! thread, and no turn re-entering another.

pub use xpute_runtime::global::Global;

/// Serializes the native checks that touch the kernel's globals: they run on
/// several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
