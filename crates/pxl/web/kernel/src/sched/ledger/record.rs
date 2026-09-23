// pxl-kernel/sched/ledger/record.rs

//! Read-only instrument for the schedulers: records where time and work go so
//! it can be seen. Never read back into a scheduling decision — the
//! observation→execution edge is one-directional on purpose
//! (issue/20260909-cooperative-scheduler-and-task-ledger.md).
//!
//! Two record types, one instrument:
//! - TaskRow — one finished unit of deferred work (a scheduler.rs
//!   demand today; a CPU task later), as the TaskRecord a scheduler settles
//!   reduced to numbers.
//! - FrameSample — one rendered frame's gauges (pacing, GPU ask, memory, queue
//!   pressure). Only accrues while frames actually render (the tick runs only
//!   when asked), which is correct: idle means nothing to sample.
//!
//! Both are laid out word for word as ledger.json says, and the rings holding
//! them are lanes the shell reads where they lie (pxl-wasm's `ledger_read`):
//! nothing is built to be read, so reading the instrument allocates nothing in
//! the heap it measures. The summary a panel or a report shows is summary.rs's,
//! written into a block of the same kind; the words for any of it are the
//! shell's. Long frames are the shell's own measurement and stay there.

use core::fmt;
use core::mem::{offset_of, size_of};

use xpute_runtime::abi::fetch::FetchFail;
use xpute_runtime::mem::heap::alloc;

use crate::global::Global;
use crate::host::now;
use crate::io::demand::{DemandKey, Phases};
use crate::sched::ledger::spec::{CATEGORY_NAMES, FRAME, GPU_SPAN_NAMES, LIMITS, OUTCOME_NAMES, PHASE_NAMES, SUMMARY, TASK, TASK_PHASE_NAMES, WORST};
use crate::sched::tick::PHASES;

pub use crate::sched::ledger::spec::CENSUS_TAGS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Aborted,
    Error,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Aborted => "aborted",
            Outcome::Error => "error",
        }
    }
}

/// A task as a scheduler settles it.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskRecord {
    /// The stream and the one thing in it.
    pub key: DemandKey,
    pub priority: f64,
    /// monotonic ms at enqueue / first run / settle.
    pub enqueue_ms: f64,
    pub start_ms: Option<f64>,
    pub end_ms: f64,
    /// 0 while inline; worker hop cost once a task body runs off-thread.
    pub transport_ms: f64,
    /// whatever split the demand's own run reported (terrain: net vs decode).
    /// None when it reported none — the run is then one opaque step.
    pub phases: Option<Phases>,
    pub outcome: Outcome,
    /// Why it failed, when `outcome` is Error. The count alone could not say
    /// whether twelve failures were one cause or twelve.
    pub fail: Failure,
}

/// Why a task failed: the reason as its number, and the HTTP status beside it
/// where the reason was a refused response. Two numbers, so a record is a row
/// of plain values and the summary counts by them rather than by prose — the
/// words for a reason are the shell's, as `strerror` is libc's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Failure {
    pub fail: FetchFail,
    pub status: u16,
}

impl fmt::Display for Failure {
    /// `status 503`, or the reason's own name where no status says more.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.fail {
            FetchFail::None => Ok(()),
            FetchFail::Status => write!(f, "status {}", self.status),
            fail => f.write_str(fail.as_str()),
        }
    }
}

/// Where `name` sits in `names`, as a record's number; -1 where it is absent.
pub fn name_id(names: &[&str], name: &str) -> f64 {
    names.iter().position(|n| *n == name).map_or(-1.0, |i| i as f64)
}

const PHASES_MAX: usize = LIMITS::PHASES_MAX as usize;
const _: () = assert!(PHASES_MAX == crate::io::demand::PHASES_MAX, "a task row holds every phase a run can report");

/// A task as the ring holds it: every word a number, as TASK lays it out.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskRow {
    /// An index into CATEGORY_NAMES.
    pub category: f64,
    pub subject: u64,
    pub priority: f64,
    pub enqueue_ms: f64,
    /// NaN for a task that never ran.
    pub start_ms: f64,
    pub end_ms: f64,
    pub transport_ms: f64,
    /// An index into OUTCOME_NAMES.
    pub outcome: f64,
    /// The FetchFail's own number, and the status beside it.
    pub fail: f64,
    pub status: f64,
    /// How many of the phase lanes are filled; 0 where the run reported none.
    pub phase_count: f64,
    /// Indices into TASK_PHASE_NAMES, a timing's -1 where the list lacks it.
    pub phase_ids: [f64; PHASES_MAX],
    pub phase_values: [f64; PHASES_MAX],
}

impl TaskRow {
    pub fn of(rec: &TaskRecord) -> TaskRow {
        let mut row = TaskRow {
            category: name_id(CATEGORY_NAMES, rec.key.category.as_str()),
            subject: rec.key.subject,
            priority: rec.priority,
            enqueue_ms: rec.enqueue_ms,
            start_ms: rec.start_ms.unwrap_or(f64::NAN),
            end_ms: rec.end_ms,
            transport_ms: rec.transport_ms,
            outcome: name_id(OUTCOME_NAMES, rec.outcome.as_str()),
            fail: rec.fail.fail as u32 as f64,
            status: rec.fail.status as f64,
            ..TaskRow::default()
        };
        for (k, (name, value)) in rec.phases.iter().flat_map(|p| p.iter()).enumerate() {
            row.phase_ids[k] = name_id(TASK_PHASE_NAMES, name);
            row.phase_values[k] = value;
            row.phase_count = (k + 1) as f64;
        }
        row
    }
}

pub const CENSUS_LEN: usize = CENSUS_TAGS.len();

/// (draws, triangles) by the pipeline a draw names (crates/pxl/web/render/src/draw.rs), for one
/// frame's render, in CENSUS_TAGS' order.
pub type DrawCensus = [[f64; 2]; CENSUS_LEN];

/// The GPU's side of a frame: its time, in all and by span; what it drew, by
/// pipeline and, of the terrain, by level; and the pixels it filled.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuSample {
    /// From the frame's first pass to its last, as the queue reports it once
    /// done: lags a frame or two, and 0 before the renderer exists or where
    /// the device keeps no time.
    pub ms: f64,
    /// That time by span, GPU_SPAN_NAMES' order (pxl-render's GPU_SPANS).
    pub spans: [f64; GPU_SPANS],
    pub draw_calls: f64,
    pub triangles: f64,
    pub census: DrawCensus,
    /// The terrain's triangles by the level its chunks are of.
    pub level_tris: [f64; LEVELS],
    /// The canvas's width*height — the raw fill-rate primitive.
    pub px: f64,
}

/// What the device holds for the frame: its buffers — the blocks, and the
/// mesh arena, which is no block's — and its targets and textures.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSample {
    /// Blocks held, and what they hold in MiB — their deltas across frames
    /// are the upload churn.
    pub buffers: f64,
    pub buffer_mb: f64,
    pub arena_mb: f64,
    pub depth_mb: f64,
    pub msaa_mb: f64,
    pub shadow_mb: f64,
    pub layers_mb: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LodSample {
    pub chunks: f64,
    pub in_frustum: f64,
    pub pending: f64,
    pub settling: f64,
    pub blend_active: f64,
    /// Pending chunks outside the frustum — releases waiting to run. They sort
    /// last by design, so this is where starvation would show first, and what
    /// it would cost is memory.
    pub offscreen_pending: f64,
    /// The longest any pending release has waited, and the longest any pending
    /// on-screen chunk has.
    pub oldest_release_ms: f64,
    pub oldest_visible_ms: f64,
    /// How far the byte budget has pulled the LOD ladder in: 0 is the ladder as
    /// the view asks for it, 2 one tier coarser.
    pub pressure: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MountSample {
    pub added: f64,
    pub removed: f64,
    pub layouts: f64,
}

/// The pages resident, every level's, and a level's own count.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PageSample {
    pub total: f64,
    pub by_level: [f64; LEVELS],
    pub poi: f64,
}

/// The heap as facts rather than one derived word: how far the allocator has
/// ever reached into its range, how much of it is out on loan right now — runs
/// at their whole size, not the slots live inside them — and how large the
/// range is; then the two ways it runs out, requests refused since boot and
/// the largest block its free lists hold.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeapSample {
    pub used_mb: f64,
    /// What the page level has handed out, a carved run counted whole: what
    /// the heap has committed, against `range_mb`.
    pub live_mb: f64,
    /// Of `live_mb`, what was borrowed to be carved into slots, and of that
    /// what is out in slots. `run_mb` less `slot_mb` is what the carving
    /// costs — a run's header and every slot standing free in a run that
    /// cannot go back until its last one does. `live_mb` less `run_mb` went
    /// straight down as whole pages.
    pub run_mb: f64,
    pub slot_mb: f64,
    pub range_mb: f64,
    pub refusals: f64,
    pub largest_free_mb: f64,
}

/// The completion ring and its spill at their fullest since the last frame: a
/// post that finds both full asserts, so this is how close the control path
/// came to that.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RingSample {
    pub cq_peak: f64,
    pub spill_peak: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkSample {
    pub scans: f64,
    pub bakes: f64,
    /// The largest mesh a bake handed over since the last frame, in boxes,
    /// of each kind of slot — a high-water mark where `scans` and `bakes`
    /// beside it are counts. It is what the slots are sized against
    /// (world/chunk/mesh_slot.rs), since a mesh past its slot is cut and the
    /// ground it held is missing.
    pub fine_peak: f64,
    pub coarse_peak: f64,
}

/// One rendered frame. Lean on purpose — latency percentiles, decode bandwidth
/// and upload cost are derived from task rows or added later.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    /// monotonic ms at record time.
    pub t: f64,
    /// rAF interval in ms — pacing, read against `frame_ms`. The gap the host
    /// measured, not the step the world took: the two part past MAX_STEP_S
    /// (sched/turn.rs), and the frames worth reading are all past it.
    pub dt_ms: f64,
    /// The display's measured frame interval when this frame was scheduled
    /// (sched/tick.rs) — what `dt_ms` has to beat.
    pub frame_ms: f64,
    /// 1 when the view had held still long enough that the tick took the
    /// large, throughput budget: a long dt on such a frame was chosen, not
    /// suffered. 0 otherwise.
    pub settled: f64,
    /// From the turn's rising edge to the end of the tick, which is what
    /// `budget_ms` is a quota on — so it holds the effects (`turn.effects_ms`)
    /// and whatever passed before the frame task was polled, as well as every
    /// phase. What `phases` sums to is the tick alone, and the difference
    /// between the two is the region nothing else names.
    pub sched_ms: f64,
    /// What the turn was allowed — derived, so it moves.
    pub budget_ms: f64,
    /// The tick by phase, in its own order, as PHASES names them.
    pub phases: [f64; PHASES.len()],
    /// The chunk scheduler's own pass, inside the visible phase.
    pub chunks_ms: f64,
    /// The kernel's whole turn: the strobe in, the tick, the draw and the
    /// submit. `dt_ms - task_ms` is then everything outside it — vsync wait,
    /// compositor, GPU, GC between frames, and the shell's own glue.
    pub task_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
    pub lod: LodSample,
    pub mounts: MountSample,
    pub pages: PageSample,
    pub heap: HeapSample,
    pub rings: RingSample,
    pub work: WorkSample,
    // network pressure — io/scheduler queued + in-flight, per category
    pub q_terrain: f64,
    pub q_poi: f64,
    pub stale: StaleSample,
    pub cap: CapSample,
    pub turn: TurnSample,
}

/// The chunks a view has moved off and that are kept until what replaces
/// them is whole (world/chunk/table.rs `retire`): where they outstay it, they
/// hide the ground that replaced them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StaleSample {
    pub chunks: f64,
    /// Of those, the ones drawn, and their levels as bits.
    pub drawn: f64,
    pub levels: f64,
    /// Wanted chunks with a layout that a drawn stale chunk hides.
    pub hidden: f64,
    /// Why the stale chunks stay: what replaces them does not cover them;
    /// covers them but is not laid out; and of those, where what is not laid
    /// out is waiting on a slot.
    pub uncovered: f64,
    pub unready: f64,
    pub on_waiter: f64,
    /// Meshes thrown away because their slot is still drawn from, and the
    /// levels of the chunks that waited and that held the slots, as bits.
    pub slot_waits: f64,
    pub wait_levels: f64,
    pub holder_levels: f64,
    /// Boxes cut from meshes that ran past the slot they go to: ground that
    /// is simply missing, and the sign that a tier outgrew what its slot was
    /// measured for (world/chunk/mesh_slot.rs).
    pub cut: f64,
}

/// The turn, in parts that add up. `task_ms` is the whole of it, and the
/// effects, the tick and the stage are what it is spent on — so what none of
/// them accounts for is visible as the difference, and a region nobody
/// measured cannot hide in it. The effects had no timing at all until a fold
/// running there took seventy milliseconds of a turn whose tick was one and
/// a half (2026-09-23).
///
/// `step_ms` is the longest single effect of the turn and `step_name` which
/// one, since a turn is only as cooperative as its longest step and a total
/// does not say where to look.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurnSample {
    pub effects_ms: f64,
    pub stage_ms: f64,
    pub step_ms: f64,
    pub step_name: f64,
}

/// What the frame's ceilings left behind. None of them is a number anyone
/// derived, so none of them is changed on an argument: this is what says
/// whether one is holding the screen back at all.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CapSample {
    /// Chunks on screen still wanting a layout that the commit pass did not
    /// reach, and chunks the selection wants that are not mounted.
    pub commit_left: f64,
    pub mount_left: f64,
    /// Whether the mount pass stopped because the table is full, rather than
    /// because the frame's own cap was spent.
    pub mount_full: f64,
    /// Pages that arrived and found every record carrying one worth more.
    pub record_refused: f64,
}

/// One of the window's worst frames by dt, kept whole with the frame before
/// it, because geometry, textures and the heap only mean anything as deltas.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorstFrame {
    pub sample: FrameSample,
    /// 1 where `prev` is the frame before; 0 for the first frame of a window.
    pub has_prev: f64,
    pub prev: FrameSample,
}

/// A record laid out as a block of ledger.json: every word a plain f64 at the
/// word that block gives it. It is what `put` may write, and the only way to
/// implement it is `words!`, which is what checks the claim — so a block
/// cannot be written by hand into an index, and a lane with no field, a field
/// with no lane and a field at the wrong word are each a compile error.
///
/// # Safety
/// The implementor is `#[repr(C)]` and every one of its words is an f64 (or
/// another such record), with no padding.
pub unsafe trait Words: Sized {}

/// `block` of the summary written from `value`, by the layout both agree on.
pub fn put<T: Words>(out: &mut [f64], block: u32, value: &T) {
    // SAFETY: Words says the record is plain f64s laid end to end, and
    // `words!` is what let it say so.
    let words = unsafe { core::slice::from_raw_parts(value as *const T as *const f64, size_of::<T>() / 8) };
    let at = block as usize;
    out[at..at + words.len()].copy_from_slice(words);
}

/// Each named field at the word ledger.json gives it, and the record marked as
/// a block's own layout.
macro_rules! words {
    ($t:ty, $words:expr, $($($field:ident).+ => $at:expr),+ $(,)?) => {
        const _: () = assert!(size_of::<$t>() == $words as usize * 8, concat!(stringify!($t), " is not the size ledger.json lays out"));
        $(const _: () = assert!(offset_of!($t, $($field).+) == $at as usize * 8, concat!(stringify!($t), " puts ", stringify!($($field).+), " off the word ledger.json gives it"));)+
        // SAFETY: the two assertions above are the claim.
        unsafe impl Words for $t {}
    };
}
pub(crate) use words;

words!(FrameSample, FRAME::WORDS,
    t => FRAME::t, dt_ms => FRAME::dt_ms, frame_ms => FRAME::frame_ms, settled => FRAME::settled,
    sched_ms => FRAME::sched_ms, budget_ms => FRAME::budget_ms, phases => FRAME::phases, chunks_ms => FRAME::chunks_ms, task_ms => FRAME::task_ms,
    gpu.ms => FRAME::gpu_ms, gpu.spans => FRAME::gpu_spans, gpu.draw_calls => FRAME::draw_calls, gpu.triangles => FRAME::triangles,
    gpu.census => FRAME::census, gpu.level_tris => FRAME::level_tris, gpu.px => FRAME::px,
    device.buffers => FRAME::buffers, device.buffer_mb => FRAME::buffer_mb, device.arena_mb => FRAME::arena_mb,
    device.depth_mb => FRAME::tex_depth_mb, device.msaa_mb => FRAME::tex_msaa_mb, device.shadow_mb => FRAME::tex_shadow_mb, device.layers_mb => FRAME::tex_layers_mb,
    lod.chunks => FRAME::lod_chunks, lod.in_frustum => FRAME::lod_in_frustum, lod.pending => FRAME::lod_pending, lod.settling => FRAME::lod_settling,
    lod.blend_active => FRAME::lod_blend_active, lod.offscreen_pending => FRAME::lod_offscreen_pending,
    lod.oldest_release_ms => FRAME::lod_oldest_release_ms, lod.oldest_visible_ms => FRAME::lod_oldest_visible_ms, lod.pressure => FRAME::lod_pressure,
    mounts.added => FRAME::mounts_added, mounts.removed => FRAME::mounts_removed, mounts.layouts => FRAME::mounts_layouts,
    pages.total => FRAME::pages_total, pages.by_level => FRAME::pages_by_level, pages.poi => FRAME::pages_poi,
    heap.used_mb => FRAME::heap_used_mb, heap.live_mb => FRAME::heap_live_mb, heap.run_mb => FRAME::heap_run_mb,
    heap.slot_mb => FRAME::heap_slot_mb, heap.range_mb => FRAME::heap_range_mb,
    heap.refusals => FRAME::heap_refusals, heap.largest_free_mb => FRAME::heap_largest_free_mb,
    rings.cq_peak => FRAME::ring_cq_peak, rings.spill_peak => FRAME::ring_spill_peak,
    work.scans => FRAME::work_scans, work.bakes => FRAME::work_bakes, work.fine_peak => FRAME::work_fine_peak, work.coarse_peak => FRAME::work_coarse_peak, q_terrain => FRAME::q_terrain, q_poi => FRAME::q_poi,
    stale.chunks => FRAME::stale_chunks, stale.drawn => FRAME::stale_drawn, stale.levels => FRAME::stale_levels, stale.hidden => FRAME::stale_hidden,
    stale.uncovered => FRAME::stale_uncovered, stale.unready => FRAME::stale_unready, stale.on_waiter => FRAME::stale_on_waiter,
    stale.slot_waits => FRAME::slot_waits, stale.wait_levels => FRAME::slot_wait_levels, stale.holder_levels => FRAME::slot_holder_levels,
    stale.cut => FRAME::slot_cut,
    cap.commit_left => FRAME::cap_commit_left, cap.mount_left => FRAME::cap_mount_left,
    cap.mount_full => FRAME::cap_mount_full, cap.record_refused => FRAME::cap_record_refused,
    turn.effects_ms => FRAME::turn_effects_ms, turn.stage_ms => FRAME::turn_stage_ms,
    turn.step_ms => FRAME::turn_step_ms, turn.step_name => FRAME::turn_step_name,
);

words!(TaskRow, TASK::WORDS,
    category => TASK::category, subject => TASK::subject, priority => TASK::priority, enqueue_ms => TASK::enqueue_ms,
    start_ms => TASK::start_ms, end_ms => TASK::end_ms, transport_ms => TASK::transport_ms, outcome => TASK::outcome,
    fail => TASK::fail, status => TASK::status, phase_count => TASK::phase_count, phase_ids => TASK::phase_ids, phase_values => TASK::phase_values,
);

words!(WorstFrame, WORST::WORDS, sample => WORST::sample, has_prev => WORST::has_prev, prev => WORST::prev);

words!(FramePeak, SUMMARY::peak::WORDS,
    present => SUMMARY::peak::present, dt_ms => SUMMARY::peak::dt_ms, sched_ms => SUMMARY::peak::sched_ms,
    chunks_ms => SUMMARY::peak::chunks_ms, gpu_ms => SUMMARY::peak::gpu_ms, draw_calls => SUMMARY::peak::draw_calls,
    triangles => SUMMARY::peak::triangles, q_terrain => SUMMARY::peak::q_terrain, q_poi => SUMMARY::peak::q_poi,
    chunks => SUMMARY::peak::chunks, in_frustum => SUMMARY::peak::in_frustum, pending => SUMMARY::peak::pending,
    settling => SUMMARY::peak::settling, blend_active => SUMMARY::peak::blend_active, pressure => SUMMARY::peak::pressure,
    offscreen_pending => SUMMARY::peak::offscreen_pending, oldest_release_ms => SUMMARY::peak::oldest_release_ms,
    oldest_visible_ms => SUMMARY::peak::oldest_visible_ms, fine_peak => SUMMARY::peak::fine_peak,
    coarse_peak => SUMMARY::peak::coarse_peak,
);

const _: () = assert!(PHASE_NAMES.len() == PHASES.len(), "ledger.json names the tick's phases");

/// The GPU's spans and the pyramid's levels, a lane of each.
pub const GPU_SPANS: usize = GPU_SPAN_NAMES.len();
pub const LEVELS: usize = LIMITS::LEVELS as usize;
const _: () = assert!(GPU_SPANS == pxl_render::gpu::renderer::GPU_SPANS.len(), "ledger.json names the renderer's spans");

#[cfg(test)]
#[path = "record.test.rs"]
mod test;
const _: () = assert!(LEVELS == pxl_spec::bake::constant::PAGE_TOP_LEVEL as usize + 1, "a lane a level holds every level of the pyramid");

/// The rings keep the last minute: a record older than WINDOW_MS is dropped as
/// a new one lands, so what the ledger holds is bounded by the display's rate,
/// not by how long the tab has been open. A report or a trace covers the minute
/// before it was taken. The rings are lanes at the capacities below — a minute
/// at 60 Hz is 3,600 frames, about 2 MB of samples — so a faster display, or a
/// minute of more tasks than a pan asks for, fills its lane before the minute
/// passes; filling one sets `truncated`.
pub const WINDOW_MS: f64 = 60_000.0;
pub const TASK_LIMIT: usize = 2_048;
pub const FRAME_LIMIT: usize = 4_096;
const WORST_CAP: usize = 12;

/// Steps `head` past the records at the front of `ring[*head..]` older than
/// `cutoff`; how many to drop from the front now — the head, once the dropped
/// part is the larger half, so a drop is a step of the head and not a shift of
/// every record.
fn trim<T>(ring: &[T], head: &mut usize, cutoff: f64, t: impl Fn(&T) -> f64) -> usize {
    while *head < ring.len() && t(&ring[*head]) < cutoff {
        *head += 1;
    }
    if *head > 0 && *head * 2 >= ring.len() {
        core::mem::take(head)
    } else {
        0
    }
}

/// `rec` onto a lane holding `len` records past its trim; when the lane is
/// full the records the window already dropped make room first, and past that
/// the ring is truncated.
fn land_lane<T: Copy>(ring: &mut [T], len: &mut usize, head: &mut usize, rec: T) -> bool {
    if *len == ring.len() && *head > 0 {
        drop_front(ring, len, core::mem::take(head));
    }
    if *len == ring.len() {
        return false;
    }
    ring[*len] = rec;
    *len += 1;
    true
}

/// The first `n` of a lane's `len` records dropped, the rest moved to the front.
pub(super) fn drop_front<T: Copy>(ring: &mut [T], len: &mut usize, n: usize) {
    ring.copy_within(n..*len, 0);
    *len -= n;
}

/// A lane for `cap` records, zeroed; none where there is no heap yet (a native
/// test), and then nothing is recorded into it.
pub(super) fn fixed_lane<T: Copy>(cap: usize) -> &'static mut [T] {
    let bytes = cap * size_of::<T>();
    let Some((at, _)) = u32::try_from(bytes).ok().and_then(alloc) else {
        // No kernel heap to take it from: a native check, where there is no
        // memory to lay out and the host's own allocator stands in. Leaked
        // once, which is what the kernel's lane is too — the ledger holds its
        // rings for the program's life either way. Without this the ledger
        // records nothing under `cargo test` and every check of what it
        // summarises reads zero.
        // SAFETY: the rings hold plain numbers, which the kernel's own lane
        // is handed zeroed below for the same reason.
        return Box::leak(vec![unsafe { core::mem::zeroed::<T>() }; cap].into_boxed_slice());
    };
    // SAFETY: a block this lane holds alone and never gives back, `bytes` long
    // and aligned past a record's alignment; zeroed, which is a valid record
    // for the plain numbers the rings hold.
    unsafe {
        core::ptr::write_bytes(at as *mut u8, 0, bytes);
        core::slice::from_raw_parts_mut(at as *mut T, cap)
    }
}

/// Live consumer for records as they land — log.rs's console stream. One sink,
/// not a list: the instrument has a single output mode at a time.
#[derive(Clone, Copy, Default)]
pub struct LedgerSink {
    pub task: Option<fn(&TaskRow)>,
    pub frame: Option<fn(&FrameSample)>,
}

/// The worst-seen values since recording started, kept as frames land so the
/// report never scans the ring for them — and past the minute the ring keeps,
/// so a stall noticed late is still on record. Reset with the recording.
///
/// Laid out as the block it is written into, and held to it by `words!` the
/// way a frame is. It used to be copied field by field into a hand-ordered
/// slice, and two lanes appended to the block's end were inserted in the
/// middle of that slice: every lane from the fifth on carried its
/// neighbour's number, and nothing said so (2026-09-23).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePeak {
    /// 1 once a frame has landed: the block reads as absent until then.
    pub present: f64,
    pub dt_ms: f64,
    pub sched_ms: f64,
    pub chunks_ms: f64,
    pub gpu_ms: f64,
    pub draw_calls: f64,
    pub triangles: f64,
    pub q_terrain: f64,
    pub q_poi: f64,
    pub chunks: f64,
    pub in_frustum: f64,
    pub pending: f64,
    pub settling: f64,
    pub blend_active: f64,
    pub pressure: f64,
    pub offscreen_pending: f64,
    pub oldest_release_ms: f64,
    pub oldest_visible_ms: f64,
    pub fine_peak: f64,
    pub coarse_peak: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecordingState {
    pub recording: bool,
    /// ms of wall time the window spans; live while recording.
    pub elapsed_ms: f64,
    pub tasks: usize,
    pub frames: usize,
    pub truncated: bool,
}

struct Ledger {
    /// The rings, lanes taken at their first record, `*_len` of each in use.
    task_ring: &'static mut [TaskRow],
    task_len: usize,
    frame_ring: &'static mut [FrameSample],
    frame_len: usize,
    /// Where each ring's live window starts: records before it are past the
    /// minute and wait for `trim` to drop them.
    task_head: usize,
    frame_head: usize,
    truncated: bool,
    sink: LedgerSink,
    /// Explicit recording window: stopping freezes the data so an export is
    /// stable while it is being read; restarting clears and begins a fresh one.
    ///
    /// Off until someone asks. Nothing in the world reads the ledger — the one
    /// caller outside the instrument writes a turn's task time back into the
    /// frame it belongs to and does nothing when there is no frame — so a
    /// client with no dev overlay pays for none of this: the rings are 2.1 MiB
    /// of frames and 360 KiB of tasks, taken on the first record and, being
    /// lanes, never given back.
    recording: bool,
    started_at: f64,
    stopped_at: Option<f64>,
    frame_peak: FramePeak,
    /// Slowest first, in a lane so they lie where they are read.
    worst: &'static mut [WorstFrame],
    worst_len: usize,
    /// The completion ring's and the spill's fullest since the last frame.
    rings: RingSample,
}

static LEDGER: Global<Ledger> = Global::new();

fn ledger() -> &'static mut Ledger {
    LEDGER.get(|| Ledger {
        task_ring: &mut [],
        task_len: 0,
        frame_ring: &mut [],
        frame_len: 0,
        task_head: 0,
        frame_head: 0,
        truncated: false,
        sink: LedgerSink::default(),
        recording: false,
        started_at: now(),
        stopped_at: None,
        frame_peak: FramePeak::default(),
        worst: &mut [],
        worst_len: 0,
        rings: RingSample::default(),
    })
}

/// True once any limit was hit and recording stopped being exhaustive.
pub fn ledger_truncated() -> bool {
    ledger().truncated
}

pub fn ledger_set_sink(sink: Option<LedgerSink>) {
    ledger().sink = sink.unwrap_or_default();
}

pub fn ledger_recording_state() -> RecordingState {
    let l = ledger();
    RecordingState {
        recording: l.recording,
        elapsed_ms: l.stopped_at.unwrap_or_else(now) - l.started_at,
        tasks: l.task_len - l.task_head,
        frames: l.frame_len - l.frame_head,
        truncated: l.truncated,
    }
}

/// Clears everything and opens a fresh window.
pub fn ledger_recording_start() {
    let l = ledger();
    (l.task_len, l.frame_len, l.worst_len) = (0, 0, 0);
    (l.task_head, l.frame_head) = (0, 0);
    l.frame_peak = FramePeak::default();
    l.truncated = false;
    l.started_at = now();
    l.stopped_at = None;
    l.recording = true;
}

/// Freezes the window; exports taken afterwards see exactly this data.
pub fn ledger_recording_stop() {
    let l = ledger();
    if !l.recording {
        return;
    }
    l.recording = false;
    l.stopped_at = Some(now());
}

pub fn ledger_record(rec: TaskRecord) {
    let l = ledger();
    if !l.recording {
        return;
    }
    let row = TaskRow::of(&rec);
    if let Some(task) = l.sink.task {
        task(&row);
    }
    if l.task_ring.is_empty() {
        l.task_ring = fixed_lane(TASK_LIMIT);
    }
    let dropped = trim(&l.task_ring[..l.task_len], &mut l.task_head, row.end_ms - WINDOW_MS, |r| r.end_ms);
    drop_front(l.task_ring, &mut l.task_len, dropped);
    if !land_lane(l.task_ring, &mut l.task_len, &mut l.task_head, row) {
        l.truncated = true;
    }
}

/// Copy of the window's worst-seen frame values.
pub fn ledger_frame_peak() -> FramePeak {
    ledger().frame_peak
}

/// The completion ring and its spill as a post left them: kept at their
/// fullest until the next frame takes them.
pub fn ledger_note_rings(cq_len: u32, spill_len: u32) {
    let r = &mut ledger().rings;
    r.cq_peak = r.cq_peak.max(cq_len as f64);
    r.spill_peak = r.spill_peak.max(spill_len as f64);
}

/// The rings' fullest since the last take, cleared by taking.
pub fn ledger_take_rings() -> RingSample {
    core::mem::take(&mut ledger().rings)
}

pub fn ledger_record_frame(s: FrameSample) {
    let l = ledger();
    if !l.recording {
        return;
    }
    if l.worst.is_empty() {
        l.worst = fixed_lane(WORST_CAP);
    }
    let at = l.worst[..l.worst_len].partition_point(|w| w.sample.dt_ms >= s.dt_ms);
    if at < l.worst.len() {
        let prev = l.frame_ring[..l.frame_len].last().copied();
        let kept = l.worst_len.min(l.worst.len() - 1);
        l.worst.copy_within(at..kept, at + 1);
        l.worst[at] = WorstFrame {
            sample: s,
            has_prev: if prev.is_some() { 1.0 } else { 0.0 },
            prev: prev.unwrap_or_default(),
        };
        l.worst_len = kept + 1;
    }

    let p = &mut l.frame_peak;
    p.present = 1.0;
    p.dt_ms = p.dt_ms.max(s.dt_ms);
    p.sched_ms = p.sched_ms.max(s.sched_ms);
    p.chunks_ms = p.chunks_ms.max(s.chunks_ms);
    p.gpu_ms = p.gpu_ms.max(s.gpu.ms);
    p.fine_peak = p.fine_peak.max(s.work.fine_peak);
    p.coarse_peak = p.coarse_peak.max(s.work.coarse_peak);
    p.draw_calls = p.draw_calls.max(s.gpu.draw_calls);
    p.triangles = p.triangles.max(s.gpu.triangles);
    p.q_terrain = p.q_terrain.max(s.q_terrain);
    p.q_poi = p.q_poi.max(s.q_poi);
    p.chunks = p.chunks.max(s.lod.chunks);
    p.in_frustum = p.in_frustum.max(s.lod.in_frustum);
    p.pending = p.pending.max(s.lod.pending);
    p.offscreen_pending = p.offscreen_pending.max(s.lod.offscreen_pending);
    p.oldest_release_ms = p.oldest_release_ms.max(s.lod.oldest_release_ms);
    p.oldest_visible_ms = p.oldest_visible_ms.max(s.lod.oldest_visible_ms);
    p.settling = p.settling.max(s.lod.settling);
    p.blend_active = p.blend_active.max(s.lod.blend_active);
    p.pressure = p.pressure.max(s.lod.pressure);

    if let Some(frame) = l.sink.frame {
        frame(&s);
    }
    if l.frame_ring.is_empty() {
        l.frame_ring = fixed_lane(FRAME_LIMIT);
    }
    let dropped = trim(&l.frame_ring[..l.frame_len], &mut l.frame_head, s.t - WINDOW_MS, |f| f.t);
    drop_front(l.frame_ring, &mut l.frame_len, dropped);
    if !land_lane(l.frame_ring, &mut l.frame_len, &mut l.frame_head, s) {
        l.truncated = true;
    }
}

/// The frame sample just recorded, so a late-arriving measurement (task_ms,
/// which is only known once the turn is over) can be written into the frame it
/// belongs to instead of being attributed to the next one.
pub fn ledger_last_frame() -> Option<&'static mut FrameSample> {
    let l = ledger();
    l.frame_ring[..l.frame_len].last_mut()
}

/// First index in a ring, ordered by `at`, whose record is at or after `t`.
fn first_at<T>(ring: &[T], t: f64, at: impl Fn(&T) -> f64) -> usize {
    ring.partition_point(|rec| at(rec) < t)
}

/// The frame window, newest last.
pub fn ledger_frames() -> &'static [FrameSample] {
    let l = ledger();
    &l.frame_ring[l.frame_head..l.frame_len]
}

/// The task window, newest last. A task is recorded as it settles, so the ring
/// is in `end_ms` order.
pub fn ledger_tasks() -> &'static [TaskRow] {
    let l = ledger();
    &l.task_ring[l.task_head..l.task_len]
}

/// The window's worst frames, slowest first.
pub fn ledger_worst() -> &'static [WorstFrame] {
    let l = ledger();
    &l.worst[..l.worst_len]
}

/// Frames recorded at or after `t`, newest last — a live reader's window, so
/// it costs the tail it reads and not the whole recording.
pub fn ledger_frames_since(t: f64) -> &'static [FrameSample] {
    let ring = ledger_frames();
    &ring[first_at(ring, t, |f| f.t)..]
}

/// Tasks settled at or after `t`.
pub fn ledger_tasks_since(t: f64) -> &'static [TaskRow] {
    let ring = ledger_tasks();
    &ring[first_at(ring, t, |r| r.end_ms)..]
}
