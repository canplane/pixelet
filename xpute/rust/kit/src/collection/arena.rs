// xpute-kit/collection/arena.rs

//! An arena is a pool of slots that are constructed once and reused. Slots are
//! handed out in order and taken back by rewinding to an earlier length.

use crate::status::errno::Errno;
use crate::status::error::MarshalError;

pub struct ArenaConfig {
    pub init_cap: u32,
    pub max_cap: u32,
}

/// `truncate` drops nothing. A slot handed out again is in whatever state
/// its last user left it, with its buffers still allocated, and the caller
/// must reinitialize it.
pub struct Arena<T: Default> {
    pub mem: Vec<T>,

    pub max_cap: u32,
    cap_of: u32,
    len_of: u32,
}

impl<T: Default> Arena<T> {
    pub fn new(opts: ArenaConfig) -> Result<Arena<T>, MarshalError> {
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
        if self.len_of >= self.cap_of {
            self.grow(self.len_of.saturating_add(1))?;
        }

        let i = self.len_of as usize;
        self.len_of += 1;
        Ok(&mut self.mem[i])
    }

    pub fn reserve(&mut self, cnt: u32) -> Result<(), MarshalError> {
        self.grow(self.len_of.saturating_add(cnt))
    }

    /// Rewinds to `new_len` slots. A `new_len` above the current length is a
    /// bug, not a way to grow.
    pub fn truncate(&mut self, new_len: u32) {
        crate::ensure!(new_len <= self.len_of, EINVAL, new_len, self.len_of);
        self.len_of = new_len;
    }

    /// The capacity doubles but stops at `max_cap`, so a request is refused
    /// only when it does not fit under `max_cap`.
    fn grow(&mut self, req: u32) -> Result<(), MarshalError> {
        if req <= self.cap_of {
            return Ok(());
        }
        if req > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW));
        }
        let new_cap = self.cap_of.saturating_mul(2).clamp(req, self.max_cap);

        self.mem.reserve(new_cap as usize - self.mem.len());
        for _ in self.cap_of..new_cap {
            self.mem.push(T::default());
        }
        self.cap_of = new_cap;
        Ok(())
    }

    /// Remembers the current length and rewinds to it when the guard drops.
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
        ArenaGuard {
            mark: arena.len(),
            arena,
        }
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
