// pxl-kernel/sys.rs

//! What all of the domain stands on: the reads (`io`), a turn's mechanics
//! (`sched`), the core memory's sections (`mem`), and what the kernel loads,
//! the table io and sched both read (`load`). Nothing here names the domain.

pub mod io;
pub mod load;
pub mod mem;
pub mod sched;

/// Native tests run on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
