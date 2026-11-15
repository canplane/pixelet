// pxl-core/link/layout.rs

//! The core's fixed ranges (sys/range.rs): `.fixed`, past the module's data
//! and before the heap, holds each owner's range at the size its consts give,
//! in the order of `Fixed`, each handed its place at boot (`install`). This
//! module also holds the bound the heap's flows are held to.

use crate::domain::layer::division;
use crate::domain::layer::terrain::fixed;
use crate::domain::layer::terrain::pool;
use crate::domain::scene::character;
use crate::domain::scene::character::Group;
use crate::domain::scene::decor::asset;
use crate::domain::scene::environment::cloud;
use crate::perf::record;
use crate::spec::manifest;
use crate::sys::heap::{PAGE_SZ, Span};
use crate::sys::io::body;
use crate::sys::io::demand::Stream;
use crate::sys::io::param;
use crate::sys::range;
use pxl_render::gpu::upload;
use xpute_kit::alloc::buddy_tree::Range;

/// Every model the core can hold at once. No model is ever freed, so this is
/// the sum of every owner's bound.
const MODEL_CAP: usize = character::MODEL_CAP + asset::MODEL_CAP + cloud::MODEL_CAP;

/// The uploads that exist once each: the ghost's, the location indicator's
/// zone, the two debug line sets (the frame gizmo's and the coverage gizmo's,
/// domain/scene/gizmo/) and the markers'.
const SINGLE_UPLOAD_CNT: usize = 5;

/// One palette upload for each of the companion's characters, the person and
/// the pet.
const CHARACTER_UPLOAD_CNT: usize = Group::ALL.len();

/// The sum of every owner's bound, so the renderer's upload table cannot
/// fill. Each bound is a count the structure fixes (the models the catalogs
/// can name, the cloud patches within reach, the characters, the singles),
/// so an overflow of the table is a bug, not load. The renderer frees a
/// list's uploads before it takes the list's new ones, so it never holds
/// more than this.
pub(crate) const LIVE_UPLOAD_CAP: usize =
    MODEL_CAP + cloud::PATCH_CAP + CHARACTER_UPLOAD_CNT + SINGLE_UPLOAD_CNT;

/// The ranges of `.fixed`, in address order. A range belongs here when it is
/// mostly in use whenever the core runs, or when its flow's memory must be
/// fixed: the body slots, the largest flow (sys/io/body.rs). Anything else
/// held only at times, or far below its worst most of the time, goes on the
/// heap (`HEAP_FLOWS_SZ`).
#[derive(Clone, Copy)]
enum Fixed {
    Terrain,
    Division,
    Bodies,
    Uploads,
}

const FIXED: [u32; 4] = [
    fixed::TERRAIN_SZ,
    division::DIVISION_SZ,
    param::BODY_SLOT_CNT * body::SLOT_SZ,
    upload::uploads_sz(LIVE_UPLOAD_CAP),
];

/// Where range `f` begins in `.fixed`: each range on a page.
const fn off(f: Fixed) -> u32 {
    let (mut at, mut k) = (0, 0);
    while k < f as usize {
        at += FIXED[k].next_multiple_of(PAGE_SZ);
        k += 1;
    }
    at
}

const FIXED_SZ: u32 = off(Fixed::Uploads) + FIXED[Fixed::Uploads as usize];

// The heap begins past the size manifest.json5 declares, since its first
// block is asked for before `install`; the declared size is the ranges' on a
// page, no more.
const _: () = assert!(
    FIXED_SZ.next_multiple_of(PAGE_SZ) == manifest::mem::FIXED_SZ,
    "manifest.json5's mem.fixed_sz is not the fixed ranges' size on a page"
);

// The stack, the fixed ranges and the heap the core counts on fit the
// memory; the boot checks the module's data with them against the memory
// given.
const _: () = assert!(
    manifest::stack::MODULE_SZ as u64
        + manifest::mem::FIXED_SZ as u64
        + manifest::mem::MIN_HEAP_SZ as u64
        <= manifest::mem::SZ as u64,
    "the stack, the fixed ranges and the heap do not fit the core's memory"
);

/// Hands each owner its range, before anything reads one, once the memory
/// given is found to hold the heap the core counts on past them; one too
/// small is a bug: the core cannot run.
pub(crate) fn install() {
    let heap = Span::end() - Span::base();
    xpute_kit::ensure!(heap >= manifest::mem::MIN_HEAP_SZ as usize, ENOMEM, heap);
    let at = |f: Fixed| range::Range::at(off(f), FIXED[f as usize]);
    fixed::RANGE.set(at(Fixed::Terrain));
    division::RANGE.set(at(Fixed::Division));
    body::RANGE.set(at(Fixed::Bodies));
    // SAFETY: the uploads' range, zeroed, the renderer's alone.
    let uploads =
        unsafe { at(Fixed::Uploads).arr::<u32>(0, FIXED[Fixed::Uploads as usize] as usize / 4) };
    upload::set_uploads(uploads, LIVE_UPLOAD_CAP);
}

/// The buddy's block for the longest body of a stream whose bodies take no
/// slot, a power of two.
const fn longest_unslotted() -> u64 {
    let mut most = 0;
    let mut k = 0;
    while k < Stream::ALL.len() {
        let s = Stream::ALL[k];
        if let Some(max) = s.max_body()
            && !s.row().slotted
            && max as u64 > most
        {
            most = max as u64;
        }
        k += 1;
    }
    most.next_power_of_two()
}

/// The caps of the heap's bounded flows added up, each in the buddy's
/// power-of-two blocks, checked against the heap the core counts on past the
/// fixed ranges. The sum is a budget, not the most live at once: some of the
/// flows share the credits, and free blocks do not add up to one block. The
/// flows:
/// - two bodies of a kind that takes no slot (sys/io/body.rs) for every credit, one out and one stopped
///   whose read the host has not answered (sys/io/fetch.rs), each freed
///   once it is decoded or answered;
/// - the ways' pool at its cap, which holds each brick's kept spans
///   (domain/layer/terrain/pool.rs, pxl-engine clipmap.rs `WAYS_POOL_WORDS`);
/// - the profiler's lanes while its setting is on (perf/record.rs).
///
/// Each flow is refused past its cap, and waits or stays absent when the heap
/// refuses it. A shard's body has no longest (`Stream::max_body`), so no sum
/// counts it. The rest of the heap is workspace.
const HEAP_FLOWS_SZ: u64 =
    2 * param::CREDIT_CAP as u64 * longest_unslotted() + pool::POOL_SZ + record::PERF_SZ;

const _: () = assert!(
    HEAP_FLOWS_SZ <= manifest::mem::MIN_HEAP_SZ as u64,
    "the caps of the heap's bounded flows do not fit the heap the core counts on"
);
