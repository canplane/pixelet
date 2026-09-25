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
//!                    pyramid.rs `ABSENT`; layer/terrain/fixed.rs). Which
//!                    page lies in which record is the pyramid's, so the
//!                    range is laid out once and its records change hands.
//!
//! A chunk as it is built, in the order its work runs.
//!
//!   .fixed.runs      the run sets a chunk's tiers are baked from, two: one
//!                    filled while the other is baked from (world/chunk/
//!                    build.rs). A set is let go once its chunk's tiers are
//!                    all on the device, which is what keeps them.
//!   .fixed.bake      the one mesh a chunk bake has open, written there and
//!                    sent to its slot on the device the moment it is done
//!                    (world/chunk/mesh_slot.rs): a mesh has no home in the
//!                    memory, since the device is where it is drawn from.
//!   .fixed.surface   the surface each mesh slot's tier is drawn with, a lane
//!                    a slot (world/chunk/mesh_slot.rs): what a thing standing
//!                    on the drawn ground reads.
//!   .fixed.decor     where one chunk's decor is gathered before it is laid
//!                    into its block (layer/decor/instance.rs).
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
//!   .fixed.ledger    the ledger's rings (sched/ledger/record.rs), which the
//!                    shell reads where they lie.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;
use xpute_runtime::mem::section::{span_of, Section};

use pxl_render::gpu::api_spec::stream;
use pxl_render::gpu::renderer::BLOCKS_BYTES;

use crate::abi::host::stream as host_stream;

use crate::layer::decor::instance::DECOR_BYTES;
use crate::layer::terrain::fixed::TERRAIN_BYTES;
use crate::mem::section::{map, PAGE_BYTES};
use crate::sched::ledger::record::LEDGER_BYTES;
use crate::world::chunk::build::{RUN_SETS, RUN_SET_BYTES};
use crate::world::chunk::mesh_slot::{BAKE_BYTES, SURFACE_BYTES};

/// A range of `.fixed`.
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
    Ledger,
}

const COUNT: usize = 8 + RUN_SETS;

impl Range {
    /// Its place in address order.
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
            Range::Ledger => 7 + RUN_SETS,
        }
    }
}

/// Each range's bytes, in address order. What a range holds is its own
/// affair; this says how many bytes it takes and nothing more.
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
    r[Range::Ledger.index()] = LEDGER_BYTES;
    r
};

/// What `.fixed` spans, which is where the heap begins past it.
pub const BYTES: u32 = span_of(&RANGES, PAGE_BYTES);

/// Range `r`, once the module has laid its memory out; none under a native
/// test, which lays none out.
pub fn range(r: Range) -> Option<Section> {
    map().map(|m| m.fixed.nth(&RANGES, PAGE_BYTES, r.index()))
}

/// Where `bytes` from `off` into range `r` lie: none under a native test, or
/// where they would run past the range's end.
pub fn at(r: Range, off: u32, bytes: u32) -> Option<u32> {
    range(r)?.at(off, bytes)
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
    unsafe { core::slice::from_raw_parts_mut(at as usize as *mut T, n) }
}
