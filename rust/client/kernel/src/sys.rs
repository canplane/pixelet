// pxl-kernel/sys.rs

//! What all of the domain stands on: the reads (`io`), a turn's mechanics
//! (`sched`), the core memory's sections (`mem`), what the kernel loads, the
//! table io and sched both read (`load`), what it was booted with (`boot`)
//! and how it stops (`stop`). Nothing here names the domain.

pub(crate) mod boot;
pub(crate) mod io;
pub(crate) mod load;
pub mod mem;
pub(crate) mod sched;
pub mod stop;

/// Native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub(crate) fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
