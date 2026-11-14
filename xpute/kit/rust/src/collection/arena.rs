// xpute-kit/collection/arena.rs

//! An arena is a pool of slots that are constructed once and reused. Slots are
//! handed out in order and taken back by rewinding to an earlier length.

use crate::status::errno::Errno;
use crate::status::error::XputeError;

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
    len: u32,
}

impl<T: Default> Arena<T> {
    pub fn new(cfg: ArenaConfig) -> Result<Arena<T>, XputeError> {
        let mut arena = Arena {
            mem: Vec::new(),
            max_cap: cfg.max_cap,
            len: 0,
        };
        arena.grow(cfg.init_cap.min(cfg.max_cap))?;
        Ok(arena)
    }

    pub fn cap(&self) -> u32 {
        self.mem.len() as u32
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&mut self, idx: u32) -> &mut T {
        &mut self.mem[idx as usize]
    }

    pub fn alloc(&mut self) -> Result<&mut T, XputeError> {
        if self.len >= self.cap() {
            self.grow(self.len.saturating_add(1))?;
        }

        let i = self.len as usize;
        self.len += 1;
        Ok(&mut self.mem[i])
    }

    pub fn reserve(&mut self, cnt: u32) -> Result<(), XputeError> {
        self.grow(self.len.saturating_add(cnt))
    }

    /// Rewinds to `new_len` slots. A `new_len` above the current length is a
    /// bug, not a way to grow.
    pub fn truncate(&mut self, new_len: u32) {
        crate::ensure!(new_len <= self.len, EINVAL, new_len, self.len);
        self.len = new_len;
    }

    /// The capacity doubles but stops at `max_cap`, so a request is refused
    /// only when it does not fit under `max_cap`.
    fn grow(&mut self, req: u32) -> Result<(), XputeError> {
        let cap = self.cap();
        if req <= cap {
            return Ok(());
        }
        if req > self.max_cap {
            return Err(XputeError::new(Errno::EOVERFLOW));
        }
        let new_cap = cap.saturating_mul(2).clamp(req, self.max_cap);
        self.mem.resize_with(new_cap as usize, T::default);
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
