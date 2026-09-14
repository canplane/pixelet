// xpute-core/ds/lanes.rs

//! A table of typed lanes that move as one: every lane has the same length,
//! a row is one index across all of them, and growing, truncating and
//! swap-removing touch every lane together.
//!
//! What this replaces was N parallel `Vector`s per table — seventeen in the
//! chunk table, eleven in the scan's scratch — each pushed, truncated and
//! swap-copied by hand in the same order, with a capacity check per lane per
//! row. One length and one capacity here; a row is claimed with `push()` and
//! written through `mem`, so no per-row object is made and no lane can drift
//! from the others.
//!
//! `mem` holds the current views and is replaced whole when the table grows,
//! so a caller reads it fresh rather than caching a lane across a push.
//!
//! The spec is a record of lane name to constructor there; here it is the
//! names and their element kinds in order, `mem` the views in that order,
//! and `lane::<T>(name)` the typed view of one.

use crate::kit::array::{AnyElement, ArrayBufferView, Kind, TypedArray};
use crate::sys::errno::Errno;
use crate::sys::error::MarshalError;

use super::typed_alloc::{TypedAlloc, HEAP_ALLOC};

pub type LaneSpec = Vec<(&'static str, Kind)>;
pub type LaneViews = Vec<ArrayBufferView>;

#[derive(Default)]
pub struct LanesOptions {
    pub init_cap: Option<u32>,
    pub max_cap: Option<u32>,
    pub alloc: Option<Box<dyn TypedAlloc>>,
}

pub struct Lanes {
    pub mem: LaneViews,
    pub max_cap: u32,
    pub spec: LaneSpec,
    alloc: Box<dyn TypedAlloc>,
    _cap: u32,
    _len: u32,
}

impl Lanes {
    pub fn new(spec: LaneSpec, opts: LanesOptions) -> Lanes {
        let init_cap = opts.init_cap.unwrap_or(1 << 10);
        let max_cap = opts.max_cap.unwrap_or(1 << 24);
        let mut alloc = opts.alloc.unwrap_or_else(|| Box::new(HEAP_ALLOC));
        let _cap = init_cap.min(max_cap);
        let mem = Self::take_of(&spec, alloc.as_mut(), _cap);
        Lanes {
            mem,
            max_cap,
            spec,
            alloc,
            _cap,
            _len: 0,
        }
    }

    pub fn len(&self) -> u32 {
        self._len
    }

    pub fn is_empty(&self) -> bool {
        self._len == 0
    }

    /// The typed view of lane `key`, as `mem[key]` reads there.
    pub fn lane<T: AnyElement>(&self, key: &str) -> TypedArray<T> {
        let i = self.spec.iter().position(|(k, _)| *k == key).unwrap_or_else(|| panic!("lanes: no lane {key}"));
        TypedArray::of(&self.mem[i]).unwrap_or_else(|| panic!("lanes: lane {key} is not of that element"))
    }

    /// Claims the next row and returns its index; the lanes hold whatever
    /// the previous occupant left, so the caller writes every lane it reads.
    pub fn push(&mut self) -> Result<u32, MarshalError> {
        if self._len >= self._cap {
            self.grow((self._cap * 2).max(1))?;
        }
        let i = self._len;
        self._len += 1;
        Ok(i)
    }

    pub fn truncate(&mut self, new_len: u32) {
        if new_len < self._len {
            self._len = new_len;
        }
    }

    pub fn clear(&mut self) {
        self._len = 0;
    }

    /// Frees row `i` by moving the last row into it. Returns the index the
    /// moved row came from (the old last), which equals `i` when `i` was last
    /// and nothing moved — so a caller keeping a handle per row knows which
    /// handle to re-point.
    pub fn swap_remove(&mut self, i: u32) -> u32 {
        self._len -= 1;
        let last = self._len;
        if i != last {
            for lane in &self.mem {
                let size = lane.kind.bytes_per_element();
                let bytes = TypedArray::<u8>::over(lane.buffer.clone(), lane.byte_offset, lane.byte_length);
                let (src, dst) = (last as usize * size, i as usize * size);
                bytes.as_mut_slice().copy_within(src..src + size, dst);
            }
        }
        last
    }

    fn take_of(spec: &LaneSpec, alloc: &mut dyn TypedAlloc, cap: u32) -> LaneViews {
        spec.iter().map(|&(_, kind)| alloc.take_view(kind, cap)).collect()
    }

    fn take(&mut self, cap: u32) -> LaneViews {
        Self::take_of(&self.spec, self.alloc.as_mut(), cap)
    }

    fn grow(&mut self, new_cap: u32) -> Result<(), MarshalError> {
        if new_cap <= self._cap {
            return Ok(());
        }
        if new_cap > self.max_cap {
            return Err(MarshalError::new(Errno::EOVERFLOW, Some(&format!("lanes: required size exceeds max_cap={}", self.max_cap)), None));
        }
        let next = self.take(new_cap);
        for (k, lane) in self.mem.iter().enumerate() {
            // The two views are always the same element: copied as bytes.
            let size = lane.kind.bytes_per_element();
            let n = self._len as usize * size;
            let from = TypedArray::<u8>::over(lane.buffer.clone(), lane.byte_offset, n);
            let to = TypedArray::<u8>::over(next[k].buffer.clone(), next[k].byte_offset, n);
            to.set(&from);
            self.alloc.give(lane);
        }
        self.mem = next;
        self._cap = new_cap;
        Ok(())
    }
}

#[cfg(test)]
#[path = "lanes.test.rs"]
mod test;
