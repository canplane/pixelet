// xpute-runtime/mem/heap.rs

//! Heap blocks named by offset, from whatever `#[global_allocator]` the
//! module installs.

use crate::mem::base::{off_of, ptr_at};

/// An offset from the base and a size: freeing needs both.
pub type Block = (usize, u32);

/// The widest lane element, `f64` or `u64`.
const BLOCK_ALIGN: usize = 8;

/// None when there is no room, and for zero bytes.
pub fn alloc(bytes: u32) -> Option<Block> {
    if bytes == 0 {
        return None;
    }
    let layout = std::alloc::Layout::from_size_align(bytes as usize, BLOCK_ALIGN).ok()?;
    // SAFETY: a layout of non-zero size.
    let at = unsafe { std::alloc::alloc(layout) };
    (!at.is_null()).then_some((off_of(at), bytes))
}

/// # Safety
/// `block` came from `alloc` and has not been given back.
pub unsafe fn free((at, bytes): Block) {
    // SAFETY: the caller's; `alloc` made this layout for this address.
    unsafe { std::alloc::dealloc(ptr_at(at), std::alloc::Layout::from_size_align_unchecked(bytes as usize, BLOCK_ALIGN)) }
}

/// Freed on drop unless `keep` hands it on.
pub struct Guard(Block);

impl Guard {
    pub fn alloc(bytes: u32) -> Option<Guard> {
        alloc(bytes).map(Guard)
    }

    pub fn keep(self) -> Block {
        let block = self.0;
        core::mem::forget(self);
        block
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: a guard is made only by `Guard::alloc`, and `keep` forgets it.
        unsafe { free(self.0) }
    }
}
