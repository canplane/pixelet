// pxl-global/lib.rs

//! A global the core's state lives in, which the core and the renderer both
//! hold. The core runs on one thread, entered only through xpute's link, one
//! entry at a time, so a global has one reader and writer at once; that
//! contract is what makes `Global` `Sync`. Nothing here is for a crate that
//! runs on more than one thread. Within an entry, a borrow `get` hands out is
//! the value's only one while it lives: no other `get` or `set` of the same
//! global, `init`'s own included, runs before it ends.

use core::cell::UnsafeCell;

pub struct Global<T>(UnsafeCell<Option<T>>);

// SAFETY: the link's contract above: one thread, one entry at a time.
unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    pub const fn new() -> Self {
        Global(UnsafeCell::new(None))
    }

    /// The value, made by `init` the first time it is asked for.
    #[expect(
        clippy::mut_from_ref,
        reason = "a borrow from `get` is the value's only one while it lives, the contract above"
    )]
    pub fn get(&self, init: impl FnOnce() -> T) -> &mut T {
        // SAFETY: the contract above: one entry at a time, and no other borrow
        // of the value while the one handed out lives.
        unsafe { (*self.0.get()).get_or_insert_with(init) }
    }

    pub fn set(&self, val: T) {
        // SAFETY: the contract above: one entry at a time, and no borrow of the
        // value from `get` alive.
        unsafe { *self.0.get() = Some(val) }
    }
}

/// A global that holds nothing yet, as `new` makes it.
impl<T> Default for Global<T> {
    fn default() -> Self {
        Global::new()
    }
}
