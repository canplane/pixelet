// xpute-runtime/mem/heap.rs

//! Blocks out of the heap: bytes asked for and given back.
//!
//! The allocator is not this module's. Which one serves is the module's own
//! choice — `#[global_allocator]`, and xpute-core's `alloc/` has two that fit
//! — and the span it serves from is one of the map's sections (section.rs).
//! What is here is the door, and the shape a block travels in.

/// A block of the heap: where it lies in the memory, and its bytes.
///
/// Two numbers rather than a `Vec` or a slice, because what holds a block
/// names it by number — a list that uploads it, a table of a chunk's
/// tiers, a host that writes into the memory at an offset — and the size
/// travels with the address, since giving a block back needs both.
pub type Block = (usize, u32);

/// Every lane element a block is cut into — bytes up to `f64`, `u64` and
/// records of them — aligns within this.
const BLOCK_ALIGN: usize = 8;

/// A block of `bytes` from the module's allocator. None when it has no room,
/// and for zero bytes, which an allocator is never asked for.
pub fn alloc(bytes: u32) -> Option<Block> {
    if bytes == 0 {
        return None;
    }
    let layout = std::alloc::Layout::from_size_align(bytes as usize, BLOCK_ALIGN).ok()?;
    // SAFETY: a layout of non-zero size.
    let at = unsafe { std::alloc::alloc(layout) };
    (!at.is_null()).then_some((at as usize, bytes))
}

/// Gives a block back.
///
/// # Safety
/// `block` came from `alloc` and has not been given back.
pub unsafe fn free((at, bytes): Block) {
    // SAFETY: the caller's; `alloc` made this layout for this address.
    unsafe { std::alloc::dealloc(at as *mut u8, std::alloc::Layout::from_size_align_unchecked(bytes as usize, BLOCK_ALIGN)) }
}

/// A block freed when it goes out of scope, as `ArenaGuard` rewinds an arena:
/// for work that can still fail after taking it. `keep` hands the block on.
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
