// pxl-kernel/mem/fixed.rs

//! What is in `.fixed`, and where each of its ranges begins.
//!
//! The map above (section.rs) knows `.fixed` as one span between the module's
//! data and the heap — how wide it is, and that it begins on a page. What is
//! inside it is here, so that adding a range is a line in this file and
//! nothing in the map.
//!
//! A range is placed once when the module boots, never freed, and of a size
//! settled at compile time. That is the whole of what makes one belong here:
//! anything that is asked for, refused or given back belongs in the heap
//! instead.
//!
//! Ranges, in the order they are laid:
//!
//!   .fixed.terrain   the resident terrain, one record a page, every record
//!                    the same size (layer/terrain/fixed.rs). Which page lies
//!                    in which record is the pyramid's (world/pyramid.rs), so
//!                    the range is laid out once and its records change hands.
//!   .fixed.bake      the one mesh a chunk bake has open, written there and
//!                    sent to its slot on the device the moment it is done
//!                    (world/chunk/mesh_slot.rs): a mesh has no home in the
//!                    memory, since the device is where it is drawn from.
//!   .fixed.runs      the run sets a chunk's tiers are baked from, two: one
//!                    filled while the other is baked from (world/chunk/
//!                    build.rs). A set is let go once its chunk's tiers are
//!                    all on the device, which is what keeps them.
//!   .fixed.gpu       the stream the renderer's calls cross to the host in
//!                    (pxl-render gpu/api.rs), written each turn from the
//!                    start.

use xpute_runtime::mem::section::{nth_at, span_of};

use pxl_render::gpu::api_spec::stream;

use crate::layer::terrain::fixed::TERRAIN_BYTES;
use crate::mem::section::{map, PAGE_BYTES};
use crate::world::chunk::build::{RUN_SETS, RUN_SET_BYTES};
use crate::world::chunk::mesh_slot::BAKE_BYTES;

/// Each range's bytes, in address order. What a range holds is its own
/// affair; this says how many bytes it takes and nothing more.
const RANGES: [u32; 4] = [TERRAIN_BYTES, BAKE_BYTES, RUN_SETS as u32 * RUN_SET_BYTES, stream::BYTES];

/// What `.fixed` spans, which is where the heap begins past it.
pub const BYTES: u32 = span_of(&RANGES, PAGE_BYTES);

/// Where the nth range begins, `base` being where `.fixed` does.
const fn range_at(base: u32, n: usize) -> u32 {
    nth_at(base, &RANGES, PAGE_BYTES, n)
}

/// Where `.fixed.terrain` lies, once the module has laid its memory out.
pub fn terrain_at() -> Option<u32> {
    map().map(|m| range_at(m.fixed.offset, 0))
}

/// Where `.fixed.bake` lies, once the module has laid its memory out.
pub fn bake_at() -> Option<u32> {
    map().map(|m| range_at(m.fixed.offset, 1))
}

/// Where run set `set` of `.fixed.runs` lies, once the module has laid its
/// memory out.
pub fn run_set_at(set: usize) -> Option<u32> {
    map().map(|m| range_at(m.fixed.offset, 2) + set as u32 * RUN_SET_BYTES)
}

/// Where `.fixed.gpu` lies, once the module has laid its memory out.
pub fn gpu_stream_at() -> Option<u32> {
    map().map(|m| range_at(m.fixed.offset, 3))
}
