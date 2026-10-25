// xpute-guest/global.rs

//! One value for the module's life, made on first use. Handing out `&mut`
//! from a static is sound only because the guest runs on one thread and a
//! turn never re-enters another. A native host marks that thread
//! (`mark_thread`), and the two readers take "none marked" each the safe way:
//! a guest asking whether it may touch a `Global` (`off_marked_thread`) takes
//! every thread for its own, and an allocator asking whether a block is the
//! guest's (`on_marked_thread`) takes none, there being no memory before boot.

use core::cell::{Cell, UnsafeCell};
use core::sync::atomic::{AtomicBool, Ordering};

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

/// Taken first by a test touching a `Global`, since tests run on several threads.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Some thread is marked as the guest's.
static MARKED: AtomicBool = AtomicBool::new(false);

thread_local! {
    // Initialized in a const block already; Android's expansion of the macro
    // is what the lint misreads as one that is not.
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static ON_MARKED: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread as the guest's, once a process, before its first turn.
pub fn mark_thread() {
    let again = MARKED.swap(true, Ordering::AcqRel);
    assert!(!again || on_marked_thread(), "a second thread marked as the guest's");
    ON_MARKED.with(|on| on.set(true));
}

/// Whether this is the thread marked as the guest's.
pub fn on_marked_thread() -> bool {
    ON_MARKED.with(Cell::get)
}

/// A thread is marked and this is another: where a `Global` must not be touched.
pub fn off_marked_thread() -> bool {
    MARKED.load(Ordering::Acquire) && !on_marked_thread()
}

impl<T> Default for Global<T> {
    fn default() -> Self {
        Self::new()
    }
}
