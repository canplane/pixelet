// xpute_core/sys/mem.rs

//! Ranges laid end to end in a linear memory: the mechanism under a kernel's
//! memory map.
//!
//! A linker script's job, and its shape. `ld` places output sections in the
//! order the script gives, each at a `. = ALIGN(n)` boundary, and answers
//! where each one landed; which sections there are, what they hold and what
//! `n` is are the script's. Nothing here knows what a range holds either — it
//! is told a base, a unit and a list of sizes.
//!
//! The central scheduler reserves the memory — one `WebAssembly.Memory` with
//! `initial == maximum`, which nothing grows — and gives it to the kernel. So
//! `memory.buffer` is one `ArrayBuffer` for the module's life: a view over it
//! stays valid, which is what lets a ring hold its words and a lane be read
//! where it lies. What the ranges are for, and how large each one is, is the
//! kernel's policy (crates/pxl/core/src/mem.rs).

/// A WebAssembly page.
pub const WASM_PAGE_BYTES: u32 = 1 << 16;

/// Words a section takes in a boot block: its offset, bytes and alignment.
pub const SECTION_WORDS: u32 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub offset: u32,
    pub bytes: u32,
    /// The boundary the section begins on, and so the one the next begins on
    /// past where this one's content ends.
    pub align: u32,
}

/// `n` at the next multiple of `unit`, which is where the range holding `n`
/// bytes ends and the one after it may begin.
pub const fn align_up(n: u32, unit: u32) -> u32 {
    n.div_ceil(unit) * unit
}

/// What `sizes` span when laid end to end, each beginning on `unit`. The last
/// one's own rounding is in here too, which is what makes this the offset of
/// the boundary whatever follows them starts on.
pub const fn span_of(sizes: &[u32], unit: u32) -> u32 {
    let (mut span, mut i) = (0, 0);
    while i < sizes.len() {
        span += align_up(sizes[i], unit);
        i += 1;
    }
    span
}

/// Where the `n`th of `sizes` begins, laid from `base` on `unit`.
pub const fn nth_at(base: u32, sizes: &[u32], unit: u32, n: usize) -> u32 {
    base + span_of(sizes.split_at(n).0, unit)
}

/// Writes a section's words at `at`.
pub fn write_section(words: &mut [u32], at: usize, s: Section) {
    words[at..at + SECTION_WORDS as usize].copy_from_slice(&[s.offset, s.bytes, s.align]);
}

/// The section whose words are at `at`.
pub fn read_section(words: &[u32], at: usize) -> Section {
    Section {
        offset: words[at],
        bytes: words[at + 1],
        align: words[at + 2],
    }
}

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

#[cfg(test)]
#[path = "mem.test.rs"]
mod test;
