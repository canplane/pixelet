// pxl-kernel/mem/section.rs

//! Which sections the memory has and where each begins:
//!
//!   module    stack first (an overflow traps), the rings, then the linker's data
//!   .fixed    ranges sized at compile time, never freed (fixed.rs)
//!   heap      everything left, one allocator
//!
//!   module    the stack at the bottom (`--stack-first`, so an overflow wraps
//!             past zero and traps), the rings at addresses pxl-wasm's
//!             build.rs fixes, and then `.rodata`, `.data` and `.bss`, which
//!             the same build.rs keeps out of the rings' way with
//!             `--global-base`. All of it the linker's or build.rs's; nothing
//!             here places any of it, and where it ends is what the linker
//!             says (`__data_end`), not a number chosen in advance.
//!   .fixed    what the kernel places once and never frees: ranges of a size
//!             known at compile time, laid end to end from the first page past
//!             the module's data. Which ranges those are is fixed.rs's, not
//!             this file's — here it is one span and its width.
//!   heap      everything left, ascending from the page past the last of the
//!             fixed ranges to the end of the memory. One allocator answers
//!             all of it — a slab of size classes over a buddy of pages
//!             (xpute-core alloc/slab.rs over alloc/buddy_tree.rs) — so a
//!             band's lanes, a fetch body and a `String` all come out of the
//!             same range and give back into it.
//!
//! **The fixed ranges are not `.bss`, and the difference is deliberate.** Both
//! are a size known at compile time, zeroed, never freed, reached by index
//! arithmetic; a 50 MiB `static [u8; N]` would land in the same place and
//! work. What it would lose is that the size is *decided* here. `.bss` is what
//! the compiler happens to put there, and a range this large is a choice about
//! how much world is resident — it should not read as compiler output, and the
//! reasoning for it should not have to be recovered from a linker map.
//!
//! Where each lands, at the two memories the shell reserves: the module,
//! then `.fixed` (mem/fixed.rs `BYTES`), then the heap, the rest. Everything
//! but the heap is the same on both: the terrain is what a view can hold, not
//! a share of what the device happens to have.
//!
//! What everything in front of the heap is checked against is MIN_HEAP_BYTES —
//! the heap a phone is guaranteed — asked once at compile time of the lowest
//! the heap could begin, and once at boot of where it actually began.
//!
//! A section begins on a page (PAGE_BYTES) and ends where its content does;
//! the next begins on the first page past that. Where a section ends is no
//! one's business but its own, so the terrain is what it holds and not a byte
//! more, and what is left over is under a page.

use xpute_runtime::mem::section::{align_up, Section};

use xpute_runtime::global::Global;

// Shared with the shell and the linker through mem.json.
pub use crate::mem::spec::{DESKTOP_BYTES, GLOBAL_BASE, MIN_HEAP_BYTES, PAGE_BYTES, PHONE_BYTES, REGION_BYTES};

pub const fn page_up(n: u32) -> u32 {
    align_up(n, PAGE_BYTES)
}

pub use crate::mem::fixed::BYTES as FIXED_BYTES;

const _: () = assert!(
    REGION_BYTES.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES) && FIXED_BYTES.is_multiple_of(PAGE_BYTES),
    "a section begins on a page, so every block in it begins on one"
);

/// The lowest the heap could begin: the linker's data cannot end before `--global-base`.
const _: () = assert!(
    GLOBAL_BASE + FIXED_BYTES + MIN_HEAP_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the module's front, the fixed ranges and the heap they are budgeted against"
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub heap: Section,
}

pub fn fixed_at(data_end: u32) -> u32 {
    page_up(data_end)
}

/// A pure function of the linker's answer: the global allocator asks for
/// its first block before anything has laid the memory out.
pub fn heap_at(data_end: u32) -> u32 {
    fixed_at(data_end) + FIXED_BYTES
}

pub fn lay_out(bytes: u32, data_end: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    let heap = heap_at(data_end);
    xpute_core::ensure!(heap + MIN_HEAP_BYTES <= regions * REGION_BYTES, ENOMEM, heap, bytes);
    Map {
        fixed: Section {
            offset: fixed_at(data_end),
            bytes: FIXED_BYTES,
            align: PAGE_BYTES,
        },
        heap: Section {
            offset: heap,
            bytes: regions * REGION_BYTES - heap,
            align: PAGE_BYTES,
        },
    }
}

static MAP: Global<Option<Map>> = Global::new();

pub fn set_map(map: Map) {
    MAP.set(Some(map));
}

/// None under a native test, which has no sections.
pub fn map() -> Option<Map> {
    *MAP.get(|| None)
}
