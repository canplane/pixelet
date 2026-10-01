// pxl-kernel/mem/fixed.rs

//! What is in `.fixed`, and where each of its ranges begins.
//!
//! The map above (section.rs) knows `.fixed` as one span between the module's
//! data and the heap — how wide it is, and that it begins on a page. What is
//! inside it is here, so that adding a range is a line in this file and
//! nothing in the map.
//!
//! A range is placed once when the module boots, never freed, and of a size
//! settled at compile time. That is the whole of what makes one belong here,
//! and a thing whose size has a bound that sits near what it holds at its
//! ordinary fill belongs here too, at its bound: the heap is what is left
//! over for what is asked for, refused and given back, and a thing that is
//! never given back only makes the heap harder to reason about.
//!
//! Whose format a thing is decides whether it can be confined here. The
//! kernel's own formats — a baked page, a slot table, a stream, a key map
//! over the assets — have bounds the design sets. An external one — a glTF,
//! a PNG, whatever an outside tool writes — has none that can be predicted,
//! so its body stays in the heap, even though the shipped assets are listed
//! with their bytes at compile time (pxl-spec asset/ingested.spec.rs): only
//! the table that names them could be here, never what they hold.
//!
//! A range is reached through `range`, `at` or `lane`, which check what is
//! asked for against the range's end — nothing adds to a range's offset by
//! itself, so nothing written through one can land in the next.
//!
//! Ranges, in the order they are laid, those that serve one thing side by
//! side:
//!
//! The ground as it is held.
//!
//!   .fixed.terrain   the resident terrain, one record a page, every record
//!                    the same size, the first of them the open ocean every
//!                    page the bake left as no object reads (world/
//!                    paging.rs `ABSENT`; layer/terrain/fixed.rs). Which
//!                    page lies in which record is residency's, so the
//!                    range is laid out once and its records change hands.
//!
//! A chunk as it is built, in the order its work runs. Its runs are not
//! here: how many a column holds is what stands in it, which no range can be
//! sized for, so they are a heap block of the size the scan counts
//! (world/chunk/build.rs).
//!
//!   .fixed.bake      the workspace: the one mesh a chunk bake has open,
//!                    written there and sent to its slot on the device the
//!                    moment it is done (world/chunk/build.rs): a mesh has
//!                    no home in the memory, since the device is where it
//!                    is drawn from.
//!   .fixed.mounts    the chunk table's own (world/chunk/table.rs): who holds
//!                    each mesh slot and what each is chained to, which slot
//!                    each mount's chunk is in, the mounts free and the chunks
//!                    mounted in order, each sized by the slots.
//!
//! The renderer's.
//!
//!   .fixed.blocks    its table of the blocks the draw list named (pxl-render
//!                    gpu/renderer.rs).
//!   .fixed.gpu       the stream its calls cross to the host in (pxl-render
//!                    gpu/api.rs), written each turn from the start.
//!
//! What the kernel says to the shell.
//!
//!   .fixed.host      the stream the kernel's other asks of the host cross in
//!                    (host.rs), the same way.
//!   .fixed.profiler  the profiler's rings (sched/profiler/record.rs), which
//!                    the shell reads where they lie.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::mem::base::ptr_at;
use xpute_runtime::mem::section::{span_of, Section};

use pxl_render::gpu::api_spec::stream;
use pxl_render::gpu::renderer::{block_slots, blocks_bytes};

use crate::abi::host::stream as host_stream;

use crate::layer::terrain::fixed::TERRAIN_BYTES;
use crate::mem::section::{map, PAGE_BYTES};
use crate::sched::profiler::record::PROFILER_BYTES;
use crate::world::chunk::build::BAKE_BYTES;
use crate::world::chunk::table::MOUNTS_BYTES;

/// A range of `.fixed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Terrain,
    Bake,
    Mounts,
    Blocks,
    Gpu,
    Host,
    Profiler,
}

const COUNT: usize = 7;

impl Range {
    /// Its place in address order.
    const fn index(self) -> usize {
        match self {
            Range::Terrain => 0,
            Range::Bake => 1,
            Range::Mounts => 2,
            Range::Blocks => 3,
            Range::Gpu => 4,
            Range::Host => 5,
            Range::Profiler => 6,
        }
    }
}

/// Models registered at most: the companion's, the cases' and decor's, each
/// bounded where it registers them. None is let go (layer/model/store.rs).
pub const MODELS_MAX: usize = crate::layer::character::companion::MODELS_MAX + crate::layer::character::preview::MODELS_MAX + crate::layer::decor::asset_store::MODELS_MAX;

/// Actors at once at most: the companion's walkers and a case's on every
/// surface.
pub const ACTORS_MAX: usize = crate::layer::character::companion::ACTORS_MAX + crate::layer::character::preview::CASES_MAX;

/// Blocks only one of which is ever held: the location puck's zone, the
/// hover ghost and the debug lines.
const SINGLE_BLOCKS: usize = 3;

/// The most blocks the renderer's table names at once: each owner's bound —
/// decor's cells (layer/decor/cell.rs `BLOCKS_MAX`), the models and the
/// actors' joint palettes, a billboard set's instances an order, and the
/// blocks one of which is held — summed, so the table cannot fill.
pub const MAX_LIVE_BLOCKS: usize = crate::layer::decor::cell::BLOCKS_MAX + MODELS_MAX + ACTORS_MAX + crate::layer::billboard::order::SETS_MAX + SINGLE_BLOCKS;

/// The table's slots, from that bound at the table's load (pxl-render
/// gpu/renderer.rs `block_slots`).
pub const BLOCK_SLOTS: usize = block_slots(MAX_LIVE_BLOCKS);

/// Each range's bytes, in address order. What a range holds is its own
/// affair; this says how many bytes it takes and nothing more.
const RANGES: [u32; COUNT] = {
    let mut r = [0; COUNT];
    r[Range::Terrain.index()] = TERRAIN_BYTES;
    r[Range::Bake.index()] = BAKE_BYTES;
    r[Range::Mounts.index()] = MOUNTS_BYTES;
    r[Range::Blocks.index()] = blocks_bytes(BLOCK_SLOTS);
    r[Range::Gpu.index()] = stream::BYTES;
    r[Range::Host.index()] = host_stream::BYTES;
    r[Range::Profiler.index()] = PROFILER_BYTES;
    r
};

/// What `.fixed` spans, which is where the heap begins past it.
pub const BYTES: u32 = span_of(&RANGES, PAGE_BYTES);

/// Range `r`. The memory is laid out as the module boots (door.rs), before
/// anything reads a range, so a range asked for before it is a bug.
pub fn range(r: Range) -> Section {
    map().or_bug(Errno::ENOTRECOVERABLE).fixed.nth(&RANGES, PAGE_BYTES, r.index())
}

/// Where `bytes` from `off` into range `r` lie: none where they would run
/// past the range's end.
pub fn at(r: Range, off: u32, bytes: u32) -> Option<u32> {
    range(r).at(off, bytes)
}

/// `n` of `T` from `off` into `s`, a range or a part of one. A lane of a size
/// settled at compile time, so one past `s`'s end or off `T`'s alignment is a
/// bug and not a refusal.
///
/// # Safety
/// The caller holds those bytes alone for as long as it keeps the lane, and
/// what lies there is a valid `T` — zeroed, as a range is when laid, for the
/// plain numbers every lane here holds.
pub unsafe fn lane<T>(s: Section, off: u32, n: usize) -> &'static mut [T] {
    let at = u32::try_from(n * size_of::<T>()).ok().and_then(|bytes| s.at(off, bytes)).or_bug(Errno::ENOTRECOVERABLE);
    xpute_core::ensure!((at as usize).is_multiple_of(align_of::<T>()), ENOTRECOVERABLE, at);
    // SAFETY: inside `s`, aligned, and the caller's alone.
    unsafe { core::slice::from_raw_parts_mut(ptr_at::<T>(at as usize), n) }
}
