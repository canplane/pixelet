// pxl_core/global.rs
//
// What a TypeScript module's top-level `let` is here: one value for the
// module's lifetime, made on first use. The kernel runs on one thread and a
// turn never re-enters another, which is what makes handing out `&mut` from
// a shared static sound; nothing here is for a second thread.
//
// The host's clock is the one thing the kernel reads from outside that is
// not a command: pxl-wasm installs it before the first turn, and a native
// test runs on the default, which starts at zero and never moves.

use core::cell::UnsafeCell;

pub struct Global<T>(UnsafeCell<Option<T>>);

// SAFETY: the kernel is single-threaded; see the module comment.
unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    pub const fn new() -> Self {
        Global(UnsafeCell::new(None))
    }

    /// The value, made by `init` the first time.
    #[allow(clippy::mut_from_ref)]
    pub fn get(&self, init: impl FnOnce() -> T) -> &mut T {
        // SAFETY: one thread, and no turn holds a reference across another.
        unsafe { (*self.0.get()).get_or_insert_with(init) }
    }

    /// Replaces the value.
    pub fn set(&self, value: T) {
        // SAFETY: as `get`.
        unsafe { *self.0.get() = Some(value) }
    }
}

impl<T> Default for Global<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub use xpute_core::sys::clock::{now, set_clock};

/// Serializes the native tests that touch the kernel's globals: `cargo test`
/// runs tests on several threads, and a `Global` is for one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
