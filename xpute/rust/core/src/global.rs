// xpute-core/global.rs

//! A `Global` is a value that lives as long as the module and is made on first
//! use. Handing out `&mut` from a static is sound only because the core runs on
//! one thread and one turn never runs inside another.
//!
//! A native runtime marks the core's thread (`mark_thread`). Until it does,
//! each question about the mark takes its safe answer. `off_marked_thread`,
//! which asks whether this thread must keep off a `Global`, answers no on
//! every thread. `on_marked_thread`, which an allocator asks to learn whether
//! a block is the core's, answers no as well, since the core has no memory
//! before boot.

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

    pub fn set(&self, val: T) {
        // SAFETY: as `get`.
        unsafe { *self.0.get() = Some(val) }
    }
}

/// A test that touches a `Global` takes this lock first: tests run on
/// several threads, and a `Global` assumes one.
#[cfg(test)]
pub fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// This says whether any thread has been marked as the core's; `ON_MARKED` says
/// whether it is this one.
static MARKED: AtomicBool = AtomicBool::new(false);

thread_local! {
    #[cfg_attr(target_os = "android", allow(clippy::missing_const_for_thread_local, reason = "already const; the lint misreads Android's expansion"))]
    static ON_MARKED: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread as the core's. A runtime calls this once per process,
/// before the core's first turn.
pub fn mark_thread() {
    let again = MARKED.swap(true, Ordering::AcqRel);
    assert!(
        !again || on_marked_thread(),
        "a second thread marked as the core's"
    );
    ON_MARKED.with(|on| on.set(true));
}

pub fn on_marked_thread() -> bool {
    ON_MARKED.with(Cell::get)
}

/// This says whether another thread is marked as the core's, so that this one
/// must not touch a `Global`.
pub fn off_marked_thread() -> bool {
    MARKED.load(Ordering::Acquire) && !on_marked_thread()
}

impl<T> Default for Global<T> {
    fn default() -> Self {
        Self::new()
    }
}
