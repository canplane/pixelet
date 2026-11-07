// pxl-kernel/door/layout.rs

//! The memory's layout past the machine's front (machine.json, xpute-core
//! machine.rs): `.fixed`, which ranges it holds, each sized by its owner and
//! handed to it at boot (`install`), and the heap after them.

use crate::domain::layer::division;
use crate::domain::layer::terrain::fixed;
use crate::domain::layer::terrain::pool;
use crate::domain::scene::character;
use crate::domain::scene::character::Group;
use crate::domain::scene::decor::asset;
use crate::domain::scene::environment::cloud;
use crate::machine::MACHINE;
use crate::perf::record;
use crate::sys::io::demand::Stream;
use crate::sys::io::param;
use crate::sys::mem::section::{self, map};
use pxl_render::gpu::api;
use pxl_render::gpu::upload;
use xpute_core::entry::BootInfo;
use xpute_core::machine::Gpu;
use xpute_core::mem::section::span_of;
use xpute_core::mem::section::{Section, PAGE_BYTES};
use xpute_kit::status::errno::Errno;

// The renderer draws through the machine's GPU device.
const GPU: Gpu = match MACHINE.gpu {
    Some(g) => g,
    None => panic!("the kernel's machine takes the GPU device"),
};

/// The ranges of `.fixed`, in address order: the resident terrain, the
/// resident divisions' cells and the renderer's upload table. Each is mostly in use whenever
/// the kernel runs, or the runtime's reservation; what is held only at times,
/// or far below its worst most of the time, is the heap's
/// (`HEAP_FLOWS_BYTES`).
#[derive(Clone, Copy)]
pub(crate) enum Fixed {
    Terrain,
    Division,
    Uploads,
}

/// None is let go.
const MODELS_MAX: usize = character::MODELS_MAX + asset::MODELS_MAX + cloud::MODELS_MAX;

/// The ghost's, the location indicator's zone, the two debug line sets' (the
/// frame gizmo's and the coverage gizmo's, domain/scene/gizmo/) and the
/// markers'.
const SINGLE_UPLOADS: usize = 5;

/// A character's palette, one for each of the companion's: the person and the pet.
const CHARACTER_UPLOADS: usize = Group::ALL.len();

/// Each owner's bound summed, so the table cannot fill: every bound is a
/// count the structure fixes (the models the catalogs can name, the clouds'
/// patches within their reach, the characters, the singles), so overflowing the
/// renderer's table is a bug, not a load.
/// The renderer frees a list's uploads before taking its new ones, so it
/// never holds more.
pub(crate) const MAX_LIVE_UPLOADS: usize =
    MODELS_MAX + cloud::PATCHES_MAX + CHARACTER_UPLOADS + SINGLE_UPLOADS;

const FIXED: [u32; 3] = [
    fixed::TERRAIN_BYTES,
    division::DIVISION_BYTES,
    upload::uploads_bytes(MAX_LIVE_UPLOADS),
];

pub const FIXED_BYTES: u32 = span_of(&FIXED, PAGE_BYTES);

// The front, the fixed ranges and the heap a phone is guaranteed fit the
// smaller size; the linker's data cannot begin before the front ends.
const _: () = MACHINE.check(FIXED_BYTES);

/// A body its kind's packet at its longest, in the block the buddy gives
/// it, a power of two; none for a kind whose bodies take slots.
const fn body_block(stream: Stream) -> u64 {
    match stream.max_body() {
        Some(_) if stream.row().slotted => 0,
        Some(max) => max.next_power_of_two() as u64,
        None => 0,
    }
}

/// The heap's bounded flows at their worst: terrain's and the Earth's
/// bodies, kept or on a credit, in the runs `BODY_SLOTS` slots fill
/// (sys/io/body.rs), and a body on every credit in the buddy's block for the
/// longest other kind, each freed as it is decoded; the ways' pool
/// at its most (domain/layer/terrain/pool.rs, pxl-engine way/tree.rs `POOL_CHILD_GROUPS`),
/// and the profiler's lanes while its setting is on (perf/record.rs), each
/// in the buddy's blocks, a power of two. Each is refused past its cap, and
/// waits or stays absent where the heap refuses it. The rest of the heap
/// is workspace, which asks first.
const HEAP_FLOWS_BYTES: u64 = {
    use crate::sys::io::body::slots_bytes;
    use crate::sys::io::demand::Stream;
    let mut longest = 0;
    let mut k = 0;
    while k < Stream::ALL.len() {
        if body_block(Stream::ALL[k]) > longest {
            longest = body_block(Stream::ALL[k]);
        }
        k += 1;
    }
    slots_bytes(param::BODY_SLOTS)
        + param::MAX_CREDITS as u64 * longest
        + pool::POOL_BYTES
        + record::PERF_BYTES
};

const _: () = assert!(
    HEAP_FLOWS_BYTES <= MACHINE.min_heap_bytes as u64,
    "the heap's bounded flows at their worst do not fit the heap a phone is guaranteed"
);

/// Asking before boot lays out memory is a bug.
pub(crate) fn range(f: Fixed) -> Section {
    use xpute_kit::status::bug::OrBug;
    map()
        .or_bug(Errno::ENOTRECOVERABLE)
        .fixed
        .nth(&FIXED, PAGE_BYTES, f as usize)
}

/// Lays the memory map and hands each range of `.fixed` to its owner, and
/// the GPU device's stream to the renderer.
pub(crate) fn install(b: &BootInfo) {
    section::set_map(section::lay_out(b.given, b.form.data_end, FIXED_BYTES));
    fixed::RANGE.set(range(Fixed::Terrain));
    division::RANGE.set(range(Fixed::Division));
    // SAFETY: `.fixed.uploads` and the GPU device's stream, zeroed, each the
    // renderer's alone.
    unsafe {
        upload::set_uploads(range(Fixed::Uploads).off, MAX_LIVE_UPLOADS);
        api::install(MACHINE.gpu_at(), GPU.bytes);
    }
}

#[cfg(test)]
#[path = "layout.test.rs"]
mod test;
