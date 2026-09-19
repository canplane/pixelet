// xpute-core/collection/arena.rs

//! Arena: a pool of constructed slots, bump-allocated and rewound whole.

use crate::status::errno::Errno;
use crate::status::error::MarshalError;
use crate::status::log;

pub struct ArenaOptions {
    /// Slots built at construction; growth passes it.
    pub init_cap: u32,
    /// Growth past it is refused.
    pub max_cap: u32,
}

impl Default for ArenaOptions {
    fn default() -> ArenaOptions {
        ArenaOptions { init_cap: 1 << 10, max_cap: 1 << 24 }
    }
}

/// Slots are built once and live as long as the arena: `truncate` moves the
/// cursor and drops nothing, so whatever a slot owns — a buffer, a string —
/// is still allocated when the slot is handed out again. For work whose
/// lifetime is one pass, a page's features or a chunk's scratch, that is one
/// rewind against a free and an allocation per object.
///
/// The cost is the caller's: a slot arrives in whatever state its last user
/// left it, and reinitializing it is theirs.
pub struct Arena<T: Default> {
    pub mem: Vec<T>,

    pub max_cap: u32,
    _cap: u32,
    _len: u32,
}

impl<T: Default> Arena<T> {
    pub fn new(opts: ArenaOptions) -> Result<Arena<T>, MarshalError> {
        let mut arena = Arena {
            mem: Vec::new(),
            max_cap: opts.max_cap,
            _cap: 0,
            _len: 0,
        };
        arena._grow(opts.init_cap.min(opts.max_cap))?;
        Ok(arena)
    }

    pub fn cap(&self) -> u32 {
        self._cap
    }

    pub fn len(&self) -> u32 {
        self._len
    }

    pub fn is_empty(&self) -> bool {
        self._len == 0
    }

    pub fn get(&mut self, idx: u32) -> &mut T {
        &mut self.mem[idx as usize]
    }

    /// Hands out the next slot.
    pub fn alloc(&mut self) -> Result<&mut T, MarshalError> {
        // Saturating so a doubling past u32 lands above max_cap and is
        // refused there, rather than wrapping under it and passing.
        if self._len >= self._cap {
            self._grow(self._cap.saturating_mul(2).max(1))?;
        }

        let i = self._len as usize;
        self._len += 1;
        Ok(&mut self.mem[i])
    }

    /// Grows now so that the next `cnt` allocs do not.
    pub fn reserve(&mut self, cnt: u32) -> Result<(), MarshalError> {
        let req = self._len.saturating_add(cnt);
        if req > self._cap {
            self._grow(self._cap.saturating_mul(2).max(req))?;
        }
        Ok(())
    }

    /// Rewinds the cursor. Slots above `new_len` stay constructed.
    ///
    /// Which `new_len` is live is the caller's to know; one above the cursor
    /// is a mistake rather than a resize request, so it is refused whole and
    /// not clamped.
    pub fn truncate(&mut self, new_len: u32) {
        if new_len > self._len {
            log::warn(&format!("[arena] truncate ignored: new_len={new_len} > len={}", self._len));
            return;
        }
        self._len = new_len;
    }

    fn _grow(&mut self, new_cap: u32) -> Result<(), MarshalError> {
        if new_cap <= self._cap {
            return Ok(());
        }
        if new_cap > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("arena: required size exceeds max_cap={}", self.max_cap)), None));
        }

        self.mem.reserve(new_cap as usize - self.mem.len());
        for _ in self._cap..new_cap {
            self.mem.push(T::default());
        }
        self._cap = new_cap;
        Ok(())
    }

    /// Marks here and rewinds to the mark when the guard drops.
    pub fn scope(&mut self) -> ArenaGuard<'_, T> {
        ArenaGuard::new(self)
    }
}

pub struct ArenaGuard<'a, T: Default> {
    _mark: u32,
    arena: &'a mut Arena<T>,
}

impl<'a, T: Default> ArenaGuard<'a, T> {
    fn new(arena: &'a mut Arena<T>) -> ArenaGuard<'a, T> {
        ArenaGuard { _mark: arena.len(), arena }
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
        self.arena.truncate(self._mark);
    }
}

#[cfg(test)]
#[path = "arena.test.rs"]
mod test;
