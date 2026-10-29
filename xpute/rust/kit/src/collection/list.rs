// xpute-kit/collection/list.rs

//! A list of at most `N`, held inline: it never grows, and a push past `N`
//! is a bug (ENOSPC). For what a program bounds by a constant, where a `Vec`
//! would grow to that bound anyway. Its first `len` slots are live.

use core::mem::MaybeUninit;

pub struct List<T, const N: usize> {
    slots: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> Default for List<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> List<T, N> {
    pub const fn new() -> Self {
        List {
            slots: [const { MaybeUninit::uninit() }; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, val: T) {
        crate::ensure!(self.len < N, ENOSPC, N);
        self.slots[self.len].write(val);
        self.len += 1;
    }

    pub fn as_slice(&self) -> &[T] {
        // SAFETY: the first `len` slots are written and not yet taken.
        unsafe { core::slice::from_raw_parts(self.slots.as_ptr().cast::<T>(), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: as `as_slice`.
        unsafe { core::slice::from_raw_parts_mut(self.slots.as_mut_ptr().cast::<T>(), self.len) }
    }

    /// The value at `at`, those after it moved down one: the order kept.
    pub fn remove(&mut self, at: usize) -> T {
        crate::ensure!(at < self.len, EINVAL, at);
        // SAFETY: `at` is live; it is read out once, and the live slots after
        // it close over it, so the first `len - 1` stay the live ones.
        unsafe {
            let p = self.slots.as_mut_ptr().cast::<T>();
            let val = core::ptr::read(p.add(at));
            core::ptr::copy(p.add(at + 1), p.add(at), self.len - at - 1);
            self.len -= 1;
            val
        }
    }

    /// Those `keep` refuses dropped, the order of the rest kept, in one pass.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let len = self.len;
        // Emptied first: a panic in `keep` leaks the rest rather than leaving
        // a slot counted twice.
        self.len = 0;
        let p = self.slots.as_mut_ptr().cast::<T>();
        let mut kept = 0;
        for at in 0..len {
            // SAFETY: slots `at..len` are live and each is visited once; a kept
            // one moves down to `kept <= at`, a refused one is dropped in place.
            unsafe {
                if keep(&*p.add(at)) {
                    if kept != at {
                        core::ptr::copy_nonoverlapping(p.add(at), p.add(kept), 1);
                    }
                    kept += 1;
                } else {
                    core::ptr::drop_in_place(p.add(at));
                }
            }
        }
        self.len = kept;
    }

    pub fn clear(&mut self) {
        let len = self.len;
        // Emptied before the drops, so a panic in one leaves nothing twice.
        self.len = 0;
        // SAFETY: the first `len` slots were live, and no longer are counted.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.slots.as_mut_ptr().cast::<T>(),
                len,
            ))
        };
    }
}

impl<T, const N: usize> Drop for List<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, const N: usize> core::ops::Deref for List<T, N> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, const N: usize> core::ops::DerefMut for List<T, N> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T, const N: usize> IntoIterator for List<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;
    fn into_iter(self) -> IntoIter<T, N> {
        IntoIter { list: self, at: 0 }
    }
}

/// A list's values by value, in order; those not reached are dropped with it.
pub struct IntoIter<T, const N: usize> {
    list: List<T, N>,
    at: usize,
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        (self.at < self.list.len).then(|| {
            // SAFETY: `at` is live and read out once; `Drop` skips it.
            let val = unsafe { self.list.slots[self.at].assume_init_read() };
            self.at += 1;
            val
        })
    }
}

impl<T, const N: usize> Drop for IntoIter<T, N> {
    fn drop(&mut self) {
        let (at, len) = (self.at, self.list.len);
        // The list forgets them all; those not yet read out are dropped here.
        self.list.len = 0;
        // SAFETY: slots `at..len` are live and were not read out.
        unsafe {
            core::ptr::drop_in_place(core::ptr::slice_from_raw_parts_mut(
                self.list.slots.as_mut_ptr().cast::<T>().add(at),
                len - at,
            ))
        };
    }
}
