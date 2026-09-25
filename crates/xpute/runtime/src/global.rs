// xpute-runtime/global.rs

//! One value for the module's lifetime, made on first use.
//!
//! The substrate runs on one thread and a turn never re-enters another, which
//! is what makes handing out `&mut` from a shared static sound. Nothing here
//! is for a second thread, and nothing here is lazy in the sense of being
//! synchronized — `get` is a null check and a call.

use core::cell::UnsafeCell;

pub struct Global<T>(UnsafeCell<Option<T>>);

// SAFETY: one thread; see the module comment.
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

/// Serializes the native checks that touch a `Global`: the test harness runs
/// them on several threads and a `Global` is for one, so two of them in one
/// static at once is not a flake but the unsoundness the type is allowed on
/// the promise of one thread. A check that names a `Global` takes this
/// first.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

impl<T> Default for Global<T> {
    fn default() -> Self {
        Self::new()
    }
}
