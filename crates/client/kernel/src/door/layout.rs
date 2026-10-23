// pxl-kernel/door/layout.rs

//! The memory's layout. Its front, below GLOBAL_BASE: the rings'
//! descriptors, their slot payloads and the directory's page, at fixed
//! addresses from mem.json. And `.fixed`: which ranges it holds, each sized
//! by its owner, and the heap after them.

use xpute_guest::ipc::door::{DoorLayout, RingLayout};
use xpute_guest::ipc::ring::{ring_bytes, slots_bytes};

use crate::mem::section::{fixed_at, map, page_up, MIN_HEAP_BYTES, PHONE_BYTES, REGION_BYTES};
use crate::mem::spec::{COMPLETION_CAPACITY, DIRECTORY_AT, GLOBAL_BASE, RING_DESC_AT, RING_SLOT_AT, STACK_BYTES, SUBMISSION_CAPACITY};
use xpute_guest::mem::section::{Section, PAGE_BYTES};

/// A packet is at most a page; anything larger crosses by address. A page
/// is also the buddy's smallest block.
pub const RING_SLOT_BYTES: u32 = PAGE_BYTES;

const SUBMISSION_DESC_AT: u32 = RING_DESC_AT;
const COMPLETION_DESC_AT: u32 = SUBMISSION_DESC_AT + page_up(ring_bytes(SUBMISSION_CAPACITY));
const DESC_END: u32 = COMPLETION_DESC_AT + page_up(ring_bytes(COMPLETION_CAPACITY));

const SUBMISSION_SLOT_AT: u32 = RING_SLOT_AT;
const COMPLETION_SLOT_AT: u32 = SUBMISSION_SLOT_AT + slots_bytes(SUBMISSION_CAPACITY, RING_SLOT_BYTES);
const SLOT_END: u32 = COMPLETION_SLOT_AT + slots_bytes(COMPLETION_CAPACITY, RING_SLOT_BYTES);

pub const DOOR_LAYOUT: DoorLayout = DoorLayout {
    slot_bytes: RING_SLOT_BYTES,
    submission: RingLayout {
        capacity: SUBMISSION_CAPACITY,
        desc_at: SUBMISSION_DESC_AT,
        slot_at: SUBMISSION_SLOT_AT,
    },
    completion: RingLayout {
        capacity: COMPLETION_CAPACITY,
        desc_at: COMPLETION_DESC_AT,
        slot_at: COMPLETION_SLOT_AT,
    },
};

// The boundaries are chosen constants, not roundings, so page alignment is asserted.
const _: () = assert!(
    RING_DESC_AT.is_multiple_of(PAGE_BYTES) && RING_SLOT_AT.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES),
    "a range begins on a page, so every slot in it begins on one"
);
const _: () = assert!(STACK_BYTES <= RING_DESC_AT, "the stack runs into the descriptors");
const _: () = assert!(DESC_END <= RING_SLOT_AT, "the descriptors run into the payloads");
const _: () = assert!(SLOT_END <= DIRECTORY_AT, "the payloads run into the directory");
const _: () = assert!(
    DIRECTORY_AT.is_multiple_of(PAGE_BYTES) && DIRECTORY_AT + PAGE_BYTES <= GLOBAL_BASE,
    "the directory is a page under the bracket's data"
);

/// The ranges of `.fixed`, in address order: the resident terrain, the
/// renderer's block table, the GPU call stream, the kernel's other host
/// calls, the profiler's rings.
#[derive(Clone, Copy)]
pub enum Fixed {
    Terrain,
    Blocks,
    Gpu,
    Host,
    Profiler,
}

/// None is let go.
const MODELS_MAX: usize = crate::scene::character::variant::MODELS_MAX + crate::decor::assets::MODELS_MAX + crate::scene::environment::cloud::MODELS_MAX;

const SINGLE_BLOCKS: usize = 3;

/// Each owner's bound summed, so the table cannot fill: every bound is a
/// count the structure fixes (a catalog's length, the camera grid's cells,
/// the surfaces), so overflowing the renderer's table is a bug, not a load.
/// The renderer frees a list's blocks before taking its uploads, so it never
/// holds more.
const MAX_LIVE_BLOCKS: usize = MODELS_MAX + crate::scene::environment::cloud::BLOCKS_MAX + crate::scene::actor::ACTORS_MAX + crate::billboard::sheet::SETS_MAX + SINGLE_BLOCKS;

pub const BLOCK_SLOTS: usize = pxl_render::gpu::blocks::block_slots(MAX_LIVE_BLOCKS);

const FIXED: [u32; 5] = [
    crate::layer::terrain::fixed::TERRAIN_BYTES,
    pxl_render::gpu::blocks::blocks_bytes(BLOCK_SLOTS),
    pxl_render::api_spec::stream::BYTES,
    crate::abi::host::stream::BYTES,
    crate::profiler::record::PROFILER_BYTES,
];

pub const FIXED_BYTES: u32 = xpute_guest::mem::section::span_of(&FIXED, PAGE_BYTES);

const _: () = assert!(
    REGION_BYTES.is_multiple_of(PAGE_BYTES) && GLOBAL_BASE.is_multiple_of(PAGE_BYTES) && FIXED_BYTES.is_multiple_of(PAGE_BYTES),
    "a section begins on a page, so every block in it begins on one"
);

/// The lowest the heap could begin: the linker's data cannot end before `--global-base`.
const _: () = assert!(
    GLOBAL_BASE + FIXED_BYTES + MIN_HEAP_BYTES <= PHONE_BYTES,
    "the phone's memory does not hold the module's front, the fixed ranges and the heap they are budgeted against"
);

/// A pure function of the linker's answer: the global allocator asks for
/// its first block before anything has laid the memory out.
pub fn heap_at(data_end: u32) -> u32 {
    fixed_at(data_end) + FIXED_BYTES
}

/// Asking before boot lays out memory is a bug.
pub fn range(f: Fixed) -> Section {
    use xpute_kit::status::bug::OrBug;
    map().or_bug(xpute_kit::status::errno::Errno::ENOTRECOVERABLE).fixed.nth(&FIXED, PAGE_BYTES, f as usize)
}
