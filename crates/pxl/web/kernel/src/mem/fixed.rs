// pxl-kernel/mem/fixed.rs

//! The ranges of `.fixed`: placed once at boot, never freed, sized at
//! compile time. Only the kernel's own formats belong here; external ones
//! (glTF, PNG) have no predictable bound and stay in the heap. Ranges are
//! reached only through `range`, `at` or `lane`, which check the range's end.
//!
//!   .fixed.terrain   resident terrain, one record per page; record 0 is `ABSENT`
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
use pxl_render::gpu::renderer::BLOCKS_BYTES;

use crate::abi::host::stream as host_stream;

use crate::layer::decor::instance::DECOR_BYTES;
use crate::layer::terrain::fixed::TERRAIN_BYTES;
use crate::mem::section::{map, PAGE_BYTES};
use crate::param::RUN_SET_BYTES;
use crate::sched::profiler::record::PROFILER_BYTES;
use crate::world::chunk::build::{BAKE_BYTES, RUN_SETS};
use crate::world::chunk::mesh_slot::SURFACE_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Terrain,
    /// Run set `n`, below RUN_SETS.
    Run(usize),
    Bake,
    Surface,
    Decor,
    Blocks,
    Gpu,
    Host,
    Profiler,
}

const COUNT: usize = 8 + RUN_SETS;

impl Range {
    const fn index(self) -> usize {
        match self {
            Range::Terrain => 0,
            Range::Run(set) => {
                // A run set past the ones there are. Const, so no bug report:
                // the panic names its place and nothing else.
                if set >= RUN_SETS {
                    panic!()
                }
                1 + set
            }
            Range::Bake => 1 + RUN_SETS,
            Range::Surface => 2 + RUN_SETS,
            Range::Decor => 3 + RUN_SETS,
            Range::Blocks => 4 + RUN_SETS,
            Range::Gpu => 5 + RUN_SETS,
            Range::Host => 6 + RUN_SETS,
            Range::Profiler => 7 + RUN_SETS,
        }
    }
}

/// None is let go.
pub const MODELS_MAX: usize = crate::layer::character::companion::MODELS_MAX + crate::layer::character::preview::MODELS_MAX + crate::layer::decor::asset_store::MODELS_MAX;

pub const ACTORS_MAX: usize = crate::layer::character::companion::ACTORS_MAX + crate::layer::character::preview::CASES_MAX;

const SINGLE_BLOCKS: usize = 3;

/// Each owner's bound summed, so the table cannot fill. The renderer frees
/// a list's blocks before taking its uploads, so it never holds more.
pub const MAX_LIVE_BLOCKS: usize = crate::layer::decor::cell::BLOCKS_MAX + MODELS_MAX + ACTORS_MAX + crate::layer::billboard::order::SETS_MAX + SINGLE_BLOCKS;

pub const BLOCK_SLOTS: usize = block_slots(MAX_LIVE_BLOCKS);

const RANGES: [u32; COUNT] = {
    let mut r = [0; COUNT];
    r[Range::Terrain.index()] = TERRAIN_BYTES;
    let mut set = 0;
    while set < RUN_SETS {
        r[Range::Run(set).index()] = RUN_SET_BYTES;
        set += 1;
    }
    r[Range::Bake.index()] = BAKE_BYTES;
    r[Range::Surface.index()] = SURFACE_BYTES;
    r[Range::Decor.index()] = DECOR_BYTES;
    r[Range::Blocks.index()] = BLOCKS_BYTES;
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
    range(r)?.at(off, bytes)
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
