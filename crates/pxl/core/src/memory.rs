// pxl_core/memory.rs

//! The kernel's policy over the memory the central scheduler gives it: which
//! sections there are and where each begins. Cutting the memory into sections
//! is the mechanism's (xpute_core::sys::memory); what a section holds, and
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
//!             the module's data. `.fixed.terrain` is the only one today — the
//!             resident terrain, one record a page, addressed by the page's
//!             own coordinate reduced modulo the set's axis
//!             (module/world/terrain/slab.rs), so nothing in it is allocated,
//!             asked for or refused.
//!   heap      everything left, ascending from the page past the last of the
//!             fixed ranges to the end of the memory. One allocator answers
//!             all of it — a slab of size classes over a buddy of pages
//!             (xpute ds/slab.rs over ds/buddy_tree.rs) — so a band's lanes, a
//!             chunk mesh and a `String` all come out of the same range and
//!             give back into it.
//!
//! **The fixed ranges are not `.bss`, and the difference is deliberate.** Both
//! are a size known at compile time, zeroed, never freed, reached by index
//! arithmetic; a 196 MiB `static [u8; N]` would land in the same place and
//! work. What it would lose is that the size is *decided* here. `.bss` is what
//! the compiler happens to put there, and a range this large is a choice about
//! how much world is resident — it should not read as compiler output, and the
//! reasoning for it should not have to be recovered from a linker map.
//!
//! Where each lands, at the two memories the shell reserves. Everything but
//! the heap is the same on both: the terrain is what a load radius reaches,
//! not a share of what the device happens to have.
//!
//!              offset         end            phone        desktop
//!   module     0              ~32.5 MiB      the linker's
//!   .fixed     ~32.5 MiB      ~228.9 MiB     196.43 MiB   196.43 MiB
//!   heap       ~228.9 MiB     —              283.08 MiB   795.08 MiB
//!
//! A section begins on a page (PAGE_BYTES) and ends where its content does;
//! the next begins on the first page past that. Where a section ends is no
//! one's business but its own, so the terrain is what it holds and not a byte
//! more, and what is left over is under a page.

use xpute_core::sys::memory::Section;

use crate::global::Global;
use crate::module::world::terrain::slab::SLAB_BYTES;

/// The memory the shell reserves, which is what the map is cut from. Not read
/// here — the kernel is handed what it was given — but written down so the two
/// sides say the same thing, and checked below against what the terrain takes.
pub const PHONE_BYTES: u32 = 512 << 20;
pub const DESKTOP_BYTES: u32 = 1024 << 20;

/// The unit the reservation is rounded down to. Nothing cuts regions any more
/// — the heap's allocator answers for its own range — so this is what a
/// device's memory is counted in and nothing else.
pub const REGION_BYTES: u32 = 1 << 26;

/// What the module's own front may take: the stack, the rings and everything
/// the linker places. **Nothing is laid out at it** — where the module's data
/// ends is the linker's answer (`__data_end`) and the fixed ranges begin at
/// the page past it, so a byte of this that goes unused goes to the heap.
///
/// It is the figure the static check below is made against. Without one, "does
/// the terrain still fit a phone" could only be asked at boot on a phone,
/// because half of it would be a link-time number; with one, the question
/// splits in two and both halves are cheap: the terrain against this at
/// compile time, and the linker against this when the module boots.
pub const MODULE_CEILING: u32 = 64 << 20;

/// What a section begins on, and the smallest block the heap's buddy hands
/// out — the same 4 KiB as xpute `ds/buddy_tree.rs`'s MIN_ALLOC, for the same
/// reason rather than by coincidence
/// (issue/20260916-the-page-table-the-pool-did-not-need.md). A buddy block
/// lands at the heap's start plus a multiple of its own size, so a heap that
/// begins on a smaller boundary hands out pages that are not pages.
pub const PAGE_BYTES: u32 = 1 << 12;

const _: () = assert!(
    PAGE_BYTES as usize == xpute_core::ds::buddy_tree::MIN_ALLOC,
    "the heap's smallest block is the page the sections are laid out on"
);

/// `.fixed`'s ranges, in the order they are laid: `.fixed.terrain` and, so
/// far, nothing else. What a range holds is its own affair — this says how
/// many bytes it takes and nothing more.
const FIXED_RANGES: [u32; 1] = [SLAB_BYTES];

/// What `.fixed` spans: each range rounded up to a page, since the next begins
/// on one. The last range's own rounding is in here too, which is what makes
/// this the offset of the page the heap starts on.
pub const FIXED_BYTES: u32 = {
    let (mut span, mut i) = (0, 0);
    while i < FIXED_RANGES.len() {
        span += FIXED_RANGES[i].div_ceil(PAGE_BYTES) * PAGE_BYTES;
        i += 1;
    }
    span
};

const _: () = assert!(
    MODULE_CEILING + FIXED_BYTES + PAGE_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the module's front, the fixed ranges and a page of heap past them"
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    pub fixed: Section,
    pub heap: Section,
}

/// Where `.fixed` begins: the first page past everything the linker placed.
pub fn fixed_at(data_end: u32) -> u32 {
    data_end.div_ceil(PAGE_BYTES) * PAGE_BYTES
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

/// Where `.fixed.terrain` lies, once the module has laid its memory out: the
/// first range, so `.fixed`'s own start.
pub fn terrain_at() -> Option<u32> {
    map().map(|m| m.fixed.offset)
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
    assert!(
        data_end <= MODULE_CEILING,
        "the module's own front outgrew what the map was checked against: the linker ended it at {data_end}, past MODULE_CEILING"
    );
    let heap = heap_at(data_end);
    assert!(
        heap + PAGE_BYTES <= regions * REGION_BYTES,
        "the memory is too small for what is fixed in it: {FIXED_BYTES} bytes of fixed ranges past {data_end}, and the heap begins after them"
    );
    Map {
        fixed: Section {
            offset: fixed_at(data_end),
            bytes: FIXED_BYTES,
            region_bytes: PAGE_BYTES,
        },
        heap: Section {
            offset: heap,
            bytes: regions * REGION_BYTES - heap,
            region_bytes: PAGE_BYTES,
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

#[cfg(test)]
#[path = "memory.test.rs"]
mod test;
