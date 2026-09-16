// xpute_core/ds/typed_alloc.rs

//! Where a typed lane's storage comes from.
//!
//! Every lane in the core — an index's columns, a batch's scratch, a mesh's
//! instance matrices — is a typed array, and each one used to be
//! `new Ctor(n)`: a buffer of its own on the JS heap. The core's data is meant
//! to live in one linear memory instead, which is the one WASM will address,
//! so structures here take their storage from an allocator rather than
//! constructing it.
//!
//! `take` keeps `new Ctor(n)`'s contract exactly — a zeroed view of `n`
//! elements — so a structure switches allocators without its own code
//! changing meaning. `HEAP_ALLOC` is `new Ctor(n)` itself and the default
//! everywhere, so nothing moves until a caller hands in another.

use crate::kit::array::{AnyElement, ArrayBuffer, ArrayBufferView, Kind, TypedArray};


pub trait TypedAlloc {
    /// A zeroed view of `length` elements of `kind`: `take` with the
    /// constructor erased, so an allocator can be held as `dyn TypedAlloc`.
    fn take_view(&mut self, kind: Kind, length: u32) -> ArrayBufferView;

    /// A zeroed view of `length` elements.
    fn take<T: AnyElement>(&mut self, length: u32) -> TypedArray<T>
    where
        Self: Sized,
    {
        TypedArray::of(&self.take_view(T::KIND, length)).expect("take_view gave a view of another element")
    }

    /// Hands back a view `take` returned. It must not be used afterwards.
    /// Accepts any view: it is only ever looked up, and a caller holding one
    /// through a library's broader type (three's attribute arrays) should not
    /// need a cast to return it.
    fn give(&mut self, view: &ArrayBufferView);
}

/// `new ctor(length)` itself.
#[derive(Clone, Copy, Default)]
pub struct HeapAlloc;

impl TypedAlloc for HeapAlloc {
    fn take_view(&mut self, kind: Kind, length: u32) -> ArrayBufferView {
        ArrayBufferView {
            buffer: ArrayBuffer::new(length as usize * kind.bytes_per_element()),
            byte_offset: 0,
            byte_length: length as usize * kind.bytes_per_element(),
            kind,
        }
    }

    fn give(&mut self, _view: &ArrayBufferView) {}
}

pub const HEAP_ALLOC: HeapAlloc = HeapAlloc;

/// Consecutive typed views over one block, cut in call order from
/// `byte_offset` on — the shape every "several spans, one allocation"
/// structure carves by hand. Views are laid where the previous one ended,
/// so a caller orders them widest element first (i32 before u8) to keep
/// each aligned to its element; the block's own start is 8-aligned. The
/// closure the TypeScript returns is a `Carver` here, `carve` its call.
pub fn carver(buf: ArrayBuffer, byte_offset: u32) -> Carver {
    Carver { buf, off: byte_offset }
}

pub struct Carver {
    buf: ArrayBuffer,
    off: u32,
}

impl Carver {
    pub fn carve<T: AnyElement>(&mut self, length: u32) -> TypedArray<T> {
        let view = TypedArray::<T>::over(self.buf.clone(), self.off as usize, length as usize);
        self.off += length * T::BYTES_PER_ELEMENT as u32;
        view
    }
}

#[cfg(test)]
#[path = "typed_alloc.test.rs"]
mod test;
