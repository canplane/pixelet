// xpute-kit/collection/arena.rs

//! Arena: a pool of constructed slots, bump-allocated and rewound whole.

use crate::status::errno::Errno;
use crate::status::error::MarshalError;

pub struct ArenaOptions {
    pub init_cap: u32,
    pub max_cap: u32,
}

impl Default for ArenaOptions {
    fn default() -> ArenaOptions {
        ArenaOptions { init_cap: 1 << 10, max_cap: 1 << 24 }
    }
}

/// `truncate` drops nothing: a slot comes back in whatever state its last
/// user left it, buffers still allocated, and the caller reinitializes it.
pub struct Arena<T: Default> {
    pub mem: Vec<T>,

    pub max_cap: u32,
    cap_of: u32,
    len_of: u32,
}

impl<T: Default> Arena<T> {
    pub fn new(opts: ArenaOptions) -> Result<Arena<T>, MarshalError> {
        let mut arena = Arena {
            mem: Vec::new(),
            max_cap: opts.max_cap,
            cap_of: 0,
            len_of: 0,
        };
        arena.grow(opts.init_cap.min(opts.max_cap))?;
        Ok(arena)
    }

    pub fn cap(&self) -> u32 {
        self.cap_of
    }

    pub fn len(&self) -> u32 {
        self.len_of
    }

    pub fn is_empty(&self) -> bool {
        self.len_of == 0
    }

    pub fn get(&mut self, idx: u32) -> &mut T {
        &mut self.mem[idx as usize]
    }

    pub fn alloc(&mut self) -> Result<&mut T, MarshalError> {
        // Saturating, so a doubling past u32 is refused rather than wrapping.
        if self.len_of >= self.cap_of {
            self.grow(self.cap_of.saturating_mul(2).max(1))?;
        }

        let i = self.len_of as usize;
        self.len_of += 1;
        Ok(&mut self.mem[i])
    }

    pub fn reserve(&mut self, cnt: u32) -> Result<(), MarshalError> {
        let req = self.len_of.saturating_add(cnt);
        if req > self.cap_of {
            self.grow(self.cap_of.saturating_mul(2).max(req))?;
        }
        Ok(())
    }

    /// A `new_len` above the cursor is a bug, not a resize.
    pub fn truncate(&mut self, new_len: u32) {
        crate::ensure!(new_len <= self.len_of, EINVAL, new_len, self.len_of);
        self.len_of = new_len;
    }

    fn grow(&mut self, new_cap: u32) -> Result<(), MarshalError> {
        if new_cap <= self.cap_of {
            return Ok(());
        }
        if new_cap > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }

        self.mem.reserve(new_cap as usize - self.mem.len());
        for _ in self.cap_of..new_cap {
            self.mem.push(T::default());
        }
        self.cap_of = new_cap;
        Ok(())
    }

    /// Marks here and rewinds to the mark when the guard drops.
    pub fn scope(&mut self) -> ArenaGuard<'_, T> {
        ArenaGuard::new(self)
    }
}

pub struct ArenaGuard<'a, T: Default> {
    mark: u32,
    arena: &'a mut Arena<T>,
}

impl<'a, T: Default> ArenaGuard<'a, T> {
    fn new(arena: &'a mut Arena<T>) -> ArenaGuard<'a, T> {
        ArenaGuard { mark: arena.len(), arena }
    }
}

impl<T: Default> core::ops::Deref for ArenaGuard<'_, T> {
    type Target = Arena<T>;
    fn deref(&self) -> &Arena<T> {
        self.arena
    }
}

impl<T: Default> core::ops::DerefMut for ArenaGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Arena<T> {
        self.arena
    }
}

impl<T: Default> Drop for ArenaGuard<'_, T> {
    fn drop(&mut self) {
        self.arena.truncate(self.mark);
    }
}

#[cfg(test)]
#[path = "arena.test.rs"]
mod test;
