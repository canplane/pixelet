// xpute-core/ds/arena.rs

//! Arena / Vector scratch allocators (bump + mark/rewind)
//! - Arena: reusable object slots
//! - Vector: reusable typed-array scratch

use crate::kit::array::{AnyElement, TypedArray};
use crate::sys::console;
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

#[derive(Default)]
pub struct ArenaOptions {
    pub init_cap: Option<u32>, // soft initial size hint
    pub max_cap: Option<u32>,  // hard size limit
}

// ============ Arena ============

/// Arena allocator with bump allocation and mark/rewind reset.
///
/// Intended for transient objects to reduce GC churn.
/// - alloc: bump cursor (O(1))
/// - truncate: rewind by mark/scope (bulk reset)
///
/// Notes:
/// - objects are preconstructed and reused
/// - alloc() does not initialize object fields
/// - caller owns object reinitialization before use
pub struct Arena<T> {
    pub mem: Vec<T>,

    pub max_cap: u32,
    _cap: u32,
    _len: u32,

    _factory: Box<dyn Fn() -> T>,
}

impl<T> Arena<T> {
    pub fn new(factory: Box<dyn Fn() -> T>, opts: ArenaOptions) -> Result<Arena<T>, MarshalError> {
        let _init_cap = opts.init_cap.unwrap_or(1 << 10);
        let max_cap = opts.max_cap.unwrap_or(1 << 24);
        let mut arena = Arena {
            mem: Vec::new(),
            max_cap,
            _cap: 0,
            _len: 0,
            _factory: factory,
        };
        arena._grow(_init_cap.min(max_cap))?;
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

    /// Allocate one object from the arena.
    pub fn alloc(&mut self) -> Result<&mut T, MarshalError> {
        // Saturating: the TypeScript's doubling is in doubles, so a size past
        // u32 reaches max_cap's refusal instead of wrapping under it.
        if self._len >= self._cap {
            self._grow(self._cap.saturating_mul(2).max(1))?;
        }

        let i = self._len as usize;
        self._len += 1;
        Ok(&mut self.mem[i])
    }

    /// Ensure space for cnt additional allocations without growing.
    pub fn reserve(&mut self, cnt: u32) -> Result<(), MarshalError> {
        let req = self._len.saturating_add(cnt);
        if req > self._cap {
            self._grow(self._cap.saturating_mul(2).max(req))?;
        }
        Ok(())
    }

    /// Rewind the bump cursor to a previous position.
    ///
    /// Kernel rule:
    /// - new_len validity is caller-owned
    /// - if new_len exceeds the current len, the request is ignored with a warning
    pub fn truncate(&mut self, new_len: u32) {
        if new_len > self._len {
            console::warn(&format!("[arena] truncate ignored: new_len={new_len} > len={}", self._len));
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
            self.mem.push((self._factory)());
        }
        self._cap = new_cap;
        Ok(())
    }

    /// Scope guard that rewinds to the current mark on dispose.
    pub fn scope(&mut self) -> ArenaGuard<'_, T> {
        ArenaGuard::new(self)
    }
}

/// Scope guard that rewinds an arena to its saved mark on dispose (drop).
pub struct ArenaGuard<'a, T> {
    _mark: u32,
    arena: &'a mut Arena<T>,
}

impl<'a, T> ArenaGuard<'a, T> {
    fn new(arena: &'a mut Arena<T>) -> ArenaGuard<'a, T> {
        ArenaGuard { _mark: arena.len(), arena }
    }
}

impl<T> core::ops::Deref for ArenaGuard<'_, T> {
    type Target = Arena<T>;
    fn deref(&self) -> &Arena<T> {
        self.arena
    }
}

impl<T> core::ops::DerefMut for ArenaGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Arena<T> {
        self.arena
    }
}

impl<T> Drop for ArenaGuard<'_, T> {
    fn drop(&mut self) {
        self.arena.truncate(self._mark);
    }
}

// ============ Vector ============

/// Contiguous typed scratch store with bump write and mark/rewind control.
///
/// Intended for transient primitive data to reduce GC churn.
/// - push: append one element (amortized O(1))
/// - set : random write with auto-grow and len extension
/// - truncate: rewind by mark/scope (bulk reset)
///
/// Notes:
/// - this is a scratch allocator, not a general container
/// - reserve(cnt) means additional capacity from the current len
/// - logical length is tracked separately from typed-array capacity
pub struct Vector<T: AnyElement> {
    pub mem: TypedArray<T>,

    pub max_cap: u32,
    _cap: u32,
    _len: u32,
}

impl<T: AnyElement> Vector<T> {
    pub fn new(opts: ArenaOptions) -> Vector<T> {
        let _init_cap = opts.init_cap.unwrap_or(1 << 10);
        let max_cap = opts.max_cap.unwrap_or(1 << 24);
        let _cap = _init_cap.min(max_cap);

        let mem = TypedArray::new(_cap as usize);
        Vector { mem, max_cap, _cap, _len: 0 }
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

    pub fn get(&self, idx: u32) -> T {
        self.mem.get(idx as usize)
    }

    /// Random write at idx.
    ///
    /// Semantics:
    /// - grows capacity if needed
    /// - extends len to idx + 1 if idx is beyond the current logical end
    pub fn set(&mut self, idx: u32, v: T) -> Result<(), MarshalError> {
        if idx >= self._cap {
            self._grow(self._cap.saturating_mul(2).max(idx.saturating_add(1)))?;
        }

        self.mem.put(idx as usize, v);
        if idx >= self._len {
            self._len = idx + 1;
        }
        Ok(())
    }

    /// Push one primitive value into scratch. Returns the written index.
    pub fn push(&mut self, v: T) -> Result<u32, MarshalError> {
        if self._len >= self._cap {
            self._grow(self._cap.saturating_mul(2).max(1))?;
        }

        let i = self._len;
        self._len += 1;
        self.mem.put(i as usize, v);
        Ok(i)
    }

    /// Ensure space for cnt additional pushes without growing.
    pub fn reserve(&mut self, cnt: u32) -> Result<(), MarshalError> {
        let req = self._len.saturating_add(cnt);
        if req > self._cap {
            self._grow(self._cap.saturating_mul(2).max(req))?;
        }
        Ok(())
    }

    /// Rewind the bump cursor to a previous position.
    ///
    /// Kernel rule:
    /// - new_len validity is caller-owned
    /// - if new_len exceeds the current len, the request is ignored with a warning
    pub fn truncate(&mut self, new_len: u32) {
        if new_len > self._len {
            console::warn(&format!("[vector] truncate ignored: new_len={new_len} > len={}", self._len));
            return;
        }
        self._len = new_len;
    }

    /// Grow the backing store and copy existing contents.
    ///
    /// Note:
    /// - this is the only allocation path after construction
    /// - if this triggers frequently, increase init_cap
    fn _grow(&mut self, new_cap: u32) -> Result<(), MarshalError> {
        if new_cap <= self._cap {
            return Ok(());
        }
        if new_cap > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("vector: required size exceeds max_cap={}", self.max_cap)), None));
        }

        let new_mem: TypedArray<T> = TypedArray::new(new_cap as usize);
        new_mem.set(&self.mem);

        self.mem = new_mem;
        self._cap = new_cap;
        Ok(())
    }

    /// Scope guard that rewinds to the current mark on dispose.
    pub fn scope(&mut self) -> VectorGuard<'_, T> {
        VectorGuard::new(self)
    }
}

pub struct VectorGuard<'a, T: AnyElement> {
    _mark: u32,
    vector: &'a mut Vector<T>,
}

impl<'a, T: AnyElement> VectorGuard<'a, T> {
    fn new(vector: &'a mut Vector<T>) -> VectorGuard<'a, T> {
        VectorGuard { _mark: vector.len(), vector }
    }
}

impl<T: AnyElement> core::ops::Deref for VectorGuard<'_, T> {
    type Target = Vector<T>;
    fn deref(&self) -> &Vector<T> {
        self.vector
    }
}

impl<T: AnyElement> core::ops::DerefMut for VectorGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Vector<T> {
        self.vector
    }
}

impl<T: AnyElement> Drop for VectorGuard<'_, T> {
    fn drop(&mut self) {
        self.vector.truncate(self._mark);
    }
}
