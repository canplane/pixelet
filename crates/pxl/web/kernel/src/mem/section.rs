// pxl-kernel/mem/section.rs

//! The kernel's policy over the memory the central scheduler gives it: which
//! sections there are and where each begins. Cutting the memory into sections
//! is the mechanism's (xpute_runtime::mem); what a section holds, and
//! what "full" means in it, is decided here
//! (issue/20260913-sections-of-the-core-memory-and-what-full-means.md).
//!
//! Sections, in address order:
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
//! Where each lands, at the two memories the shell reserves. Everything but
//! the heap is the same on both: the terrain is what a view can hold,
//! not a share of what the device happens to have.
//!
//! ```text
//!              offset         end            phone        desktop
//!   module     0              ~32.7 MiB      the linker's
//!   .fixed     ~32.7 MiB      ~84.2 MiB      51.51 MiB    51.51 MiB
//!   heap       ~84.2 MiB      —              427.79 MiB   939.79 MiB
//! ```
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

use crate::global::Global;

// The numbers the map is made of are mem.json's, since the shell and the
// linker read the same ones (mem/mem.spec.rs).
pub use crate::mem::spec::{DESKTOP_BYTES, GLOBAL_BASE, MIN_HEAP_BYTES, PAGE_BYTES, PHONE_BYTES, REGION_BYTES};

/// `n` at the page. The kernel's one application of xpute's `align_up`: the
/// unit every range in this memory begins on is PAGE_BYTES and nothing else,
/// so the choice is made here once rather than passed at each call.
pub const fn page_up(n: u32) -> u32 {
    align_up(n, PAGE_BYTES)
}

/// What `.fixed` spans, which is what the heap begins past. Whose ranges add
/// up to it is fixed.rs's business.
pub use crate::mem::fixed::BYTES as FIXED_BYTES;

const _: () = assert!(
    REGION_BYTES.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES) && FIXED_BYTES.is_multiple_of(PAGE_BYTES),
    "a section begins on a page, so every block in it begins on one"
);

/// The lowest `.heap` could possibly begin: the linker cannot end its data
/// before `--global-base`, and the fixed ranges follow it. Both are constants,
/// so the guarantee can be asked of them at compile time.
const _: () = assert!(
    GLOBAL_BASE + FIXED_BYTES + MIN_HEAP_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the module's front, the fixed ranges and the heap they are budgeted against"
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub heap: Section,
}

/// Where `.fixed` begins: the first page past everything the linker placed.
pub fn fixed_at(data_end: u32) -> u32 {
    page_up(data_end)
}

/// Where the heap begins: the first page past the last fixed range.
///
/// A function of the linker's answer and the constants above, and of nothing
/// else — no state, no order to be in. Which it has to be: the module's global
/// allocator is asked for its first block by whatever runs first, well before
/// anything has laid the memory out.
pub fn heap_at(data_end: u32) -> u32 {
    fixed_at(data_end) + FIXED_BYTES
}

/// The map over a memory of `bytes`, whose module data the linker ended at
/// `data_end`: the fixed ranges from the page past that, and the heap from the
/// page past the last of them to the end of the memory, ascending.
///
/// Which is the whole of the rule. What is fixed is settled at boot and never
/// moves; the heap is everything that is not, it starts where they stop, and
/// it is the only part of the map whose size is what the device happened to
/// give rather than what the content needs.
pub fn lay_out(bytes: u32, data_end: u32) -> Map {
    let regions = bytes / REGION_BYTES;
    let heap = heap_at(data_end);
    // What is fixed in this memory leaves the heap less than it is budgeted:
    // where the heap began, against the memory the device gave.
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

/// Records the map the module laid its memory out in.
pub fn set_map(map: Map) {
    MAP.set(Some(map));
}

/// The map, once the module has laid its memory out; none under a native
/// test, which has no sections.
pub fn map() -> Option<Map> {
    *MAP.get(|| None)
}
