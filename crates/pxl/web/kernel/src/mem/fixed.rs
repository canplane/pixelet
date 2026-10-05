// pxl-kernel/mem/fixed.rs

//! The ranges of `.fixed`: placed once at boot, never freed, sized at
//! compile time. Only the kernel's own formats belong here; external ones
//! (glTF, PNG) have no predictable bound and stay in the heap. Ranges are
//! reached only through `range`, `at` or `lane`, which check the range's end.
//!
//!   .fixed.terrain   resident terrain, one record per framed page
//!   .fixed.bake      the one open bake's mesh, before it goes to its slot
//!   .fixed.mounts    the chunk table's lanes
//!   .fixed.blocks    the renderer's block table
//!   .fixed.gpu       the GPU call stream to the host
//!   .fixed.host      the kernel's other host calls
//!   .fixed.profiler  the profiler's rings, read in place by the shell

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

/// None is let go.
pub const MODELS_MAX: usize = crate::layer::character::variant::MODELS_MAX + crate::layer::decor::asset_store::MODELS_MAX;

pub const ACTORS_MAX: usize = crate::layer::character::companion::ACTORS_MAX + crate::layer::character::preview::CASES_MAX;

const SINGLE_BLOCKS: usize = 3;

/// Each owner's bound summed, so the table cannot fill. The renderer frees
/// a list's blocks before taking its uploads, so it never holds more.
pub const MAX_LIVE_BLOCKS: usize = crate::layer::decor::cell::BLOCKS_MAX + MODELS_MAX + ACTORS_MAX + crate::layer::billboard::order::SETS_MAX + SINGLE_BLOCKS;

pub const BLOCK_SLOTS: usize = block_slots(MAX_LIVE_BLOCKS);

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

pub const BYTES: u32 = span_of(&RANGES, PAGE_BYTES);

/// Asking before boot lays out memory is a bug.
pub fn range(r: Range) -> Section {
    map().or_bug(Errno::ENOTRECOVERABLE).fixed.nth(&RANGES, PAGE_BYTES, r.index())
}

/// None where the bytes would run past the range's end.
pub fn at(r: Range, off: u32, bytes: u32) -> Option<u32> {
    range(r).at(off, bytes)
}

/// `n` of `T` from `off` into `s`. Sizes are fixed at compile time, so out
/// of bounds or misaligned is a bug, not a refusal.
///
/// # Safety
/// The caller holds those bytes alone while it keeps the lane, and they are
/// a valid `T` (zeroed, for the plain numbers every lane here holds).
pub unsafe fn lane<T>(s: Section, off: u32, n: usize) -> &'static mut [T] {
    let at = u32::try_from(n * size_of::<T>()).ok().and_then(|bytes| s.at(off, bytes)).or_bug(Errno::ENOTRECOVERABLE);
    xpute_core::ensure!((at as usize).is_multiple_of(align_of::<T>()), ENOTRECOVERABLE, at);
    // SAFETY: inside `s`, aligned, and the caller's alone.
    unsafe { core::slice::from_raw_parts_mut(ptr_at::<T>(at as usize), n) }
}
