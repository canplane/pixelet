// pxl-kernel/global.rs

//! The substrate's one-value-per-module cell. Sound because the module runs
//! on one thread and no turn re-enters another.

pub use xpute_runtime::global::Global;

/// Native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
