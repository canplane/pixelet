// @xpute/core/ds/typed_alloc.ts

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

use core::cell::RefCell;
use std::rc::Rc;

use crate::kit::array::{AnyElement, ArrayBuffer, ArrayBufferView, Kind, TypedArray};

use super::buddy::BuddyAllocator;

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

/// An allocator whose storage can also be named by a number. A lane holds the
/// handle and a view is made from it when needed — so a record is numbers
/// only, and the view it was using can be dropped and made again.
pub trait AddressedAlloc: TypedAlloc {
    /// The handle of the storage a view `take` returned sits over.
    fn handle_of(&self, view: &ArrayBufferView) -> u32;
    /// A view of `length` elements over the storage `handle` names, as a
    /// view of `kind`; `view_of` types it.
    fn view_of_kind(&self, kind: Kind, handle: u32, length: u32) -> ArrayBufferView;
    /// A view of `length` elements over the storage `handle` names.
    fn view_of<T: AnyElement>(&self, handle: u32, length: u32) -> TypedArray<T>
    where
        Self: Sized,
    {
        TypedArray::of(&self.view_of_kind(T::KIND, handle, length)).expect("view_of_kind gave a view of another element")
    }
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

/// The allocator `buddy_alloc` makes: it holds the pool with whoever else
/// does, as the TypeScript closure holds it.
struct BuddyAlloc {
    pool: Rc<RefCell<BuddyAllocator>>,
}

impl TypedAlloc for BuddyAlloc {
    fn take_view(&mut self, kind: Kind, length: u32) -> ArrayBufferView {
        let bytes = length * kind.bytes_per_element() as u32;
        let block = self.pool.borrow_mut().acquire(bytes).expect("pool: acquire refused");
        // A pool block keeps its last borrower's bytes. Zeroed through a byte
        // view because `fill(0)` throws on the BigInt arrays.
        TypedArray::<u8>::over(block.buf.clone(), block.byte_offset as usize, bytes as usize).fill(0);
        ArrayBufferView {
            buffer: block.buf,
            byte_offset: block.byte_offset as usize,
            byte_length: bytes as usize,
            kind,
        }
    }

    fn give(&mut self, view: &ArrayBufferView) {
        self.pool.borrow_mut().release_at(&view.buffer, view.byte_offset as u32, view.byte_length as u32);
    }
}

impl AddressedAlloc for BuddyAlloc {
    fn handle_of(&self, view: &ArrayBufferView) -> u32 {
        self.pool.borrow().handle_at(&view.buffer, view.byte_offset as u32, view.byte_length as u32).expect("pool: no handle")
    }

    fn view_of_kind(&self, kind: Kind, handle: u32, length: u32) -> ArrayBufferView {
        let (buf, byte_offset) = self.pool.borrow().handle_base(handle);
        ArrayBufferView {
            buffer: buf,
            byte_offset: byte_offset as usize,
            byte_length: length as usize * kind.bytes_per_element(),
            kind,
        }
    }
}

/// Views carved from a buddy pool, one block each.
///
/// Given back by address rather than by identity: `give` frees whatever block
/// a view sits over, so a view made again from a handle frees the same block
/// the original would have. A view's own byte length is the size it was
/// taken at, which is how the pool recovers the block's order.
pub fn buddy_alloc(pool: Rc<RefCell<BuddyAllocator>>) -> impl AddressedAlloc {
    BuddyAlloc { pool }
}

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
