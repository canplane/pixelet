// xpute-guest/global.rs

//! One value for the module's life, made on first use. Handing out `&mut`
//! from a static is sound only because the module runs on one thread and a
//! turn never re-enters another.

use core::cell::UnsafeCell;

pub struct Global<T>(UnsafeCell<Option<T>>);

// SAFETY: one thread; see the module comment.
unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    pub const fn new() -> Self {
        Global(UnsafeCell::new(None))
    }

    #[allow(clippy::mut_from_ref)]
    pub fn get(&self, init: impl FnOnce() -> T) -> &mut T {
        // SAFETY: one thread, and no turn holds a reference across another.
        unsafe { (*self.0.get()).get_or_insert_with(init) }
    }

    pub fn set(&self, value: T) {
        // SAFETY: as `get`.
        unsafe { *self.0.get() = Some(value) }
    }
}

/// A test touching a `Global` takes this first: the harness runs tests on
/// several threads, and two in one `Global` at once is unsound.
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
