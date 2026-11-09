// pxl-core/door/layout.rs

//! How memory past the machine's front is laid out (machine.json, xpute-core
//! machine.rs). `.fixed` comes first and holds ranges that each owner sizes
//! and receives at boot (`install`). The heap follows.

use crate::domain::layer::division;
use crate::domain::layer::terrain::fixed;
use crate::domain::layer::terrain::pool;
use crate::domain::scene::character;
use crate::domain::scene::character::Group;
use crate::domain::scene::decor::asset;
use crate::domain::scene::environment::cloud;
use crate::perf::record;
use crate::sys::io::demand::Stream;
use crate::sys::io::param;
use crate::sys::machine::MACHINE;
use crate::sys::section;
use pxl_render::gpu::api;
use pxl_render::gpu::upload;
use xpute_core::entry::BootInfo;
use xpute_core::machine::Gpu;
use xpute_core::mem::section::span_of;
use xpute_core::mem::section::{Section, PAGE_BYTES};
use xpute_kit::status::errno::Errno;

// The core's machine must take the GPU device, because the renderer draws
// through it.
const GPU: Gpu = match MACHINE.gpu {
    Some(g) => g,
    None => panic!("the core's machine takes the GPU device"),
};

/// The ranges of `.fixed`, in address order. A range belongs here only when
/// it is mostly in use whenever the core runs; anything else would waste the
/// memory the runtime reserves. What is held only at times, or far below its
/// worst most of the time, goes on the heap (`HEAP_FLOWS_BYTES`).
#[derive(Clone, Copy)]
pub(crate) enum Fixed {
    Terrain,
    Division,
    Uploads,
}

/// Every model the core can hold at once. No model is ever freed, so this is
/// the sum of every owner's bound.
const MODELS_MAX: usize = character::MODELS_MAX + asset::MODELS_MAX + cloud::MODELS_MAX;

/// The uploads that exist once each: the ghost's, the location indicator's
/// zone, the two debug line sets (the frame gizmo's and the coverage gizmo's,
/// domain/scene/gizmo/) and the markers'.
const SINGLE_UPLOADS: usize = 5;

/// One palette upload for each of the companion's characters, the person and
/// the pet.
const CHARACTER_UPLOADS: usize = Group::ALL.len();

/// The sum of every owner's bound, so the renderer's upload table cannot
/// fill. Each bound is a count the structure fixes (the models the catalogs
/// can name, the cloud patches within reach, the characters, the singles),
/// so an overflow of the table is a bug, not load. The renderer frees a
/// list's uploads before it takes the list's new ones, so it never holds
/// more than this.
pub(crate) const MAX_LIVE_UPLOADS: usize =
    MODELS_MAX + cloud::PATCHES_MAX + CHARACTER_UPLOADS + SINGLE_UPLOADS;

const FIXED: [u32; 3] = [
    fixed::TERRAIN_BYTES,
    division::DIVISION_BYTES,
    upload::uploads_bytes(MAX_LIVE_UPLOADS),
];

pub const FIXED_BYTES: u32 = span_of(&FIXED, PAGE_BYTES);

// The front, the fixed ranges and the heap a phone is guaranteed must fit in
// the smallest memory size, and the linker's data must not begin before the
// front ends.
const _: () = MACHINE.check(FIXED_BYTES);

/// The buddy allocator's block for the longest body of `stream`'s kind, a
/// power of two. It is 0 for a kind whose bodies go in slots or that has no
/// longest body.
const fn body_block(stream: Stream) -> u64 {
    match stream.max_body() {
        Some(_) if stream.row().slotted => 0,
        Some(max) => max.next_power_of_two() as u64,
        None => 0,
    }
}

/// The caps of the heap's bounded flows added up, checked against the heap a
/// phone is guaranteed. The sum is a budget, not the most live at once: some
/// of the flows share the credits, and free blocks do not add up to one block.
/// The flows:
/// - the terrain's and the Earth's bodies, kept or on a credit, in the runs
///   that `BODY_SLOTS` slots fill (sys/io/body.rs);
/// - two baked tile's bodies for every credit, one out and one stopped whose
///   read the runtime has not passed (sys/io/fetch.rs `Queue`), in the buddy's
///   block for the longest other kind, or a terrain body too long for its slot
///   (sys/io/body.rs `take`) in one no longer, each freed once it is decoded
///   or passed;
/// - the ways' pool at its largest (domain/layer/terrain/pool.rs, pxl-engine
///   way_tree.rs `POOL_CHILD_GROUPS`);
/// - the profiler's lanes while its setting is on (perf/record.rs).
///
/// The pool and the lanes sit in the buddy's power-of-two blocks. Each flow is
/// refused past its cap, and waits or stays absent when the heap refuses it. A
/// shard's body has no longest (`Stream::max_body`), so no sum counts it.
///
/// The rest of the heap is workspace.
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
        + 2 * param::MAX_CREDITS as u64 * longest
        + pool::POOL_BYTES
        + record::PERF_BYTES
};

const _: () = assert!(
    HEAP_FLOWS_BYTES <= MACHINE.min_heap_bytes as u64,
    "the caps of the heap's bounded flows do not fit the heap a phone is guaranteed"
);

/// The range `f` of `.fixed`. Asking for it before boot lays out memory is a
/// bug.
pub(crate) fn range(f: Fixed) -> Section {
    use xpute_kit::status::bug::OrBug;
    section::fixed()
        .or_bug(Errno::ENOTRECOVERABLE)
        .nth(&FIXED, PAGE_BYTES, f as usize)
}

pub(crate) fn install(b: &BootInfo) {
    section::set_fixed(section::lay_out(b.mem_sz, b.form.data_end, FIXED_BYTES));
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
