// pxl-kernel/door/layout.rs

//! The memory's layout. Its front, below GLOBAL_BASE: the rings'
//! descriptors, their slot payloads and the directory's page, at fixed
//! addresses from mem.json. And `.fixed`: which ranges it holds, each sized
//! by its owner and handed to it at boot (`install`), and the heap after
//! them.

use xpute_guest::entry::BootInfo;
use xpute_guest::ipc::ring::{ring_bytes, slots_bytes, Ring};

use crate::sys::mem::section::{
    self, fixed_at, map, page_up, MIN_HEAP_BYTES, PHONE_BYTES, REGION_BYTES,
};
use crate::sys::mem::spec::{
    CQ_CAP, DIRECTORY_AT, GLOBAL_BASE, RING_DESC_AT, RING_SLOT_AT, SQ_CAP, STACK_BYTES,
};
use xpute_guest::mem::section::{Section, PAGE_BYTES};

/// A packet is at most a page; anything larger crosses by address. A page
/// is also the buddy's smallest block.
pub const RING_SLOT_BYTES: u32 = PAGE_BYTES;

const SQ_DESC_AT: u32 = RING_DESC_AT;
const CQ_DESC_AT: u32 = SQ_DESC_AT + page_up(ring_bytes(SQ_CAP));
const DESC_END: u32 = CQ_DESC_AT + page_up(ring_bytes(CQ_CAP));

const SQ_SLOT_AT: u32 = RING_SLOT_AT;
const CQ_SLOT_AT: u32 = SQ_SLOT_AT + slots_bytes(SQ_CAP, RING_SLOT_BYTES);
const SLOT_END: u32 = CQ_SLOT_AT + slots_bytes(CQ_CAP, RING_SLOT_BYTES);

/// The door's two rings, submission and completion, laid out at their
/// addresses in the module's memory.
///
/// # Safety
/// Once: the ranges are the rings' alone, in memory that never moves and
/// outlives them.
pub unsafe fn rings() -> (Ring, Ring) {
    let mem = xpute_guest::mem::core();
    // SAFETY: the caller's.
    unsafe {
        (
            Ring::init(
                mem,
                SQ_DESC_AT as usize,
                SQ_CAP,
                SQ_SLOT_AT,
                RING_SLOT_BYTES,
            ),
            Ring::init(
                mem,
                CQ_DESC_AT as usize,
                CQ_CAP,
                CQ_SLOT_AT,
                RING_SLOT_BYTES,
            ),
        )
    }
}

// The boundaries are chosen constants, not roundings, so page alignment is asserted.
const _: () = assert!(
    RING_DESC_AT.is_multiple_of(PAGE_BYTES)
        && RING_SLOT_AT.is_multiple_of(PAGE_BYTES)
        && GLOBAL_BASE.is_multiple_of(PAGE_BYTES),
    "a range begins on a page, so every slot in it begins on one"
);
const _: () = assert!(
    STACK_BYTES <= RING_DESC_AT,
    "the stack runs into the descriptors"
);
const _: () = assert!(
    DESC_END <= RING_SLOT_AT,
    "the descriptors run into the payloads"
);
const _: () = assert!(
    SLOT_END <= DIRECTORY_AT,
    "the payloads run into the directory"
);
const _: () = assert!(
    DIRECTORY_AT.is_multiple_of(PAGE_BYTES) && DIRECTORY_AT + PAGE_BYTES <= GLOBAL_BASE,
    "the directory is a page under the bracket's data"
);

/// The ranges of `.fixed`, in address order: the resident terrain, the
/// resident divisions' cells, the renderer's upload table, the GPU call
/// stream, the kernel's other host calls. Each is mostly in use whenever
/// the kernel runs, or the host's reservation; what is held only at times,
/// or far below its worst most of the time, is the heap's
/// (`HEAP_FLOWS_BYTES`).
#[derive(Clone, Copy)]
pub enum Fixed {
    Terrain,
    Division,
    Uploads,
    Gpu,
    Host,
}

/// None is let go.
const MODELS_MAX: usize = crate::domain::scene::character::variant::MODELS_MAX
    + crate::domain::query::decor::asset::MODELS_MAX
    + crate::domain::scene::environment::cloud::MODELS_MAX;

/// The ghost's, the puck's zone, the two debug line sets' (the gizmo's and the
/// coverage's, `domain/scene.rs` `debug` and `coverage`) and the markers'.
const SINGLE_UPLOADS: usize = 5;

/// An actor's palette, one for each frame that holds one: the companion's
/// figures and the display cases.
const ACTOR_UPLOADS: usize = crate::domain::scene::character::companion::FIGURES
    + crate::domain::scene::character::preview::CASES_MAX;

/// Each owner's bound summed, so the table cannot fill: every bound is a
/// count the structure fixes (the models the catalogs can name, the clouds'
/// patches within their reach, the actors, the singles), so overflowing the
/// renderer's table is a bug, not a load.
/// The renderer frees a list's uploads before taking its new ones, so it
/// never holds more.
pub const MAX_LIVE_UPLOADS: usize = MODELS_MAX
    + crate::domain::scene::environment::cloud::PATCHES_MAX
    + ACTOR_UPLOADS
    + SINGLE_UPLOADS;

const FIXED: [u32; 5] = [
    crate::domain::layer::terrain::fixed::TERRAIN_BYTES,
    crate::domain::layer::division::DIVISION_BYTES,
    pxl_render::gpu::upload::uploads_bytes(MAX_LIVE_UPLOADS),
    pxl_render::api_spec::stream::BYTES,
    crate::sys::io::host_spec::stream::BYTES,
];

pub const FIXED_BYTES: u32 = xpute_guest::mem::section::span_of(&FIXED, PAGE_BYTES);

const _: () = assert!(
    REGION_BYTES.is_multiple_of(PAGE_BYTES)
        && GLOBAL_BASE.is_multiple_of(PAGE_BYTES)
        && FIXED_BYTES.is_multiple_of(PAGE_BYTES),
    "a section begins on a page, so every block in it begins on one"
);

/// The lowest the heap could begin: the linker's data cannot end before `--global-base`.
const _: () = assert!(
    GLOBAL_BASE + FIXED_BYTES + MIN_HEAP_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the module's front, the fixed ranges and the heap they are budgeted against"
);

/// A body its kind's packet at its longest, in the block the buddy gives
/// it, a power of two; none for a kind whose bodies take slots.
const fn body_block(stream: crate::sys::io::demand::Stream) -> u64 {
    match stream.max_body() {
        Some(_) if crate::sys::io::body::is_slotted(stream) => 0,
        Some(max) => max.next_power_of_two() as u64,
        None => 0,
    }
}

/// The heap's bounded flows at their worst: terrain's and the Earth's
/// bodies, kept or on a credit, in the runs `BODY_SLOTS` slots fill
/// (sys/io/body.rs), and a body on every credit in the buddy's block for the
/// longest other kind, each freed as it is decoded; the ways' pool's
/// mirror at its most (domain/layer/terrain/mirror.rs, pxl-spec-core way/tree.rs `POOL_FAMILIES`),
/// the renderer's rooms for the bricks' records, every read's at a
/// brick's most (pxl-render gpu/ray/ground.rs `GroundBuffers::room`), and
/// the profiler's lanes while its setting is on (profiler/record.rs), each
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
    let ways = crate::domain::layer::terrain::mirror::MIRROR_BYTES
        + pxl_render::gpu::renderer::RECORD_ROOMS_BYTES;
    slots_bytes(crate::sys::io::param::BODY_SLOTS)
        + crate::sys::io::param::MAX_CREDITS as u64 * longest
        + ways
        + crate::profiler::record::PROFILER_BYTES
};

const _: () = assert!(
    HEAP_FLOWS_BYTES <= MIN_HEAP_BYTES as u64,
    "the heap's bounded flows at their worst do not fit the heap a phone is guaranteed"
);

/// A pure function of the linker's answer: the global allocator asks for
/// its first block before anything has laid the memory out.
pub fn heap_at(data_end: u32) -> u32 {
    fixed_at(data_end) + FIXED_BYTES
}

/// Asking before boot lays out memory is a bug.
pub fn range(f: Fixed) -> Section {
    use xpute_kit::status::bug::OrBug;
    map()
        .or_bug(xpute_kit::status::errno::Errno::ENOTRECOVERABLE)
        .fixed
        .nth(&FIXED, PAGE_BYTES, f as usize)
}

/// Lays the memory map and hands each range of `.fixed` to its owner.
/// Returns where GPU calls are recorded for the host, 0 where the kernel
/// makes them itself.
pub fn install(b: &BootInfo) -> u32 {
    section::set_map(section::lay_out(b.given, b.data_end, FIXED_BYTES));
    crate::domain::layer::terrain::fixed::RANGE.set(range(Fixed::Terrain));
    crate::domain::layer::division::RANGE.set(range(Fixed::Division));
    // SAFETY: `.fixed.gpu`, `.fixed.host` and `.fixed.uploads`, zeroed, each the
    // renderer's or the host's alone.
    unsafe {
        crate::sys::io::host::set_stream(range(Fixed::Host).offset);
        pxl_render::gpu::upload::set_uploads(range(Fixed::Uploads).offset, MAX_LIVE_UPLOADS);
        pxl_render::gpu::api::install(range(Fixed::Gpu).offset)
    }
}

#[cfg(test)]
#[path = "layout.test.rs"]
mod test;
