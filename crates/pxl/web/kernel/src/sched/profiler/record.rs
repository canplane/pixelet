// pxl-kernel/sched/profiler/record.rs

//! Read-only instrument: records where time and work go. Never read back
//! into a scheduling decision. Records are laid out word for word as
//! profiler.json says, in lanes the shell reads in place, so reading the
//! profiler allocates nothing in the heap it measures.

use core::fmt;
use core::mem::{offset_of, size_of};
use core::panic::Location;
use std::collections::HashMap;

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_runtime::mem::base::off_of_slice;
use xpute_runtime::mem::section::span_of;

use crate::io::demand::{DemandKey, Phases};
use crate::mem::fixed::{lane, range, Range};
use crate::sched::profiler::log::PENDING;
use crate::sched::profiler::spec::{FRAME, GPU_SPAN_NAMES, LIMITS, PHASE_NAMES, SUMMARY, TASK, WORST};
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::space::adapter::epsg3857::zoom_of;
use pxl_spec::space::pyramid::{GROUND, TOP_LEVEL};
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;
use xpute_runtime::sched::tick::PHASES;

pub use crate::sched::profiler::spec::OutcomeName as Outcome;
pub use crate::sched::profiler::spec::CENSUS_TAGS;

#[derive(Clone, Debug, PartialEq)]
pub struct TaskRecord {
    pub key: DemandKey,
    pub priority: f64,
    pub enqueue_ms: f64,
    pub start_ms: Option<f64>,
    pub end_ms: f64,
    /// None when the run reported no split.
    pub phases: Option<Phases>,
    pub outcome: Outcome,
    pub fail: Failure,
}

/// Why a task failed, as numbers the summary can count by. A kernel-made
/// failure carries its source location: EBADMSG alone does not say which
/// rule a body broke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Failure {
    pub errno: i32,
    pub detail: u16,
    pub at: Option<&'static Location<'static>>,
}

impl From<Errno> for Failure {
    #[track_caller]
    fn from(errno: Errno) -> Failure {
        Failure {
            errno: errno as i32,
            detail: 0,
            at: Some(Location::caller()),
        }
    }
}

impl From<MarshalError> for Failure {
    fn from(err: MarshalError) -> Failure {
        Failure {
            errno: err.errno as i32,
            detail: 0,
            at: Some(err.at),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.errno, self.detail) {
            (0, _) => Ok(()),
            (errno, 0) => write!(f, "errno {errno}"),
            (errno, detail) => write!(f, "errno {errno} {detail}"),
        }
    }
}

const PHASES_MAX: usize = LIMITS::PHASES_MAX as usize;
const _: () = assert!(PHASES_MAX == crate::io::demand::PHASES_MAX, "a task row holds every phase a run can report");

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskRow {
    pub category: f64,
    pub subject: u64,
    pub priority: f64,
    pub enqueue_ms: f64,
    /// NaN for a task that never ran.
    pub start_ms: f64,
    pub end_ms: f64,
    pub outcome: f64,
    pub fail: f64,
    pub detail: f64,
    /// Failure location: file name address and length, and line. 0 when the host raised it.
    pub at_file: f64,
    pub at_file_bytes: f64,
    pub at_line: f64,
    pub phase_count: f64,
    pub phase_ids: [f64; PHASES_MAX],
    pub phase_values: [f64; PHASES_MAX],
}

impl TaskRow {
    pub fn of(rec: &TaskRecord) -> TaskRow {
        let mut row = TaskRow {
            category: rec.key.category as u32 as f64,
            subject: rec.key.subject,
            priority: rec.priority,
            enqueue_ms: rec.enqueue_ms,
            start_ms: rec.start_ms.unwrap_or(f64::NAN),
            end_ms: rec.end_ms,
            outcome: rec.outcome as u32 as f64,
            fail: rec.fail.errno as f64,
            detail: rec.fail.detail as f64,
            ..TaskRow::default()
        };
        if let Some(at) = rec.fail.at {
            row.at_file = off_of_slice(file_in_memory(at.file()).as_bytes()) as f64;
            row.at_file_bytes = at.file().len() as f64;
            row.at_line = at.line() as f64;
        }
        for (k, (name, value)) in rec.phases.iter().flat_map(|p| p.iter()).enumerate() {
            row.phase_ids[k] = name as u32 as f64;
            row.phase_values[k] = value;
            row.phase_count = (k + 1) as f64;
        }
        row
    }
}

static FILES: Global<HashMap<usize, Box<str>>> = Global::new();

/// A location's file name lies outside kernel memory on a device, so rows
/// name a heap copy; the names are few and kept for good.
fn file_in_memory(file: &'static str) -> &'static str {
    FILES.get(HashMap::new).entry(file.as_ptr().addr()).or_insert_with(|| file.into())
}

pub const CENSUS_LEN: usize = CENSUS_TAGS.len();

/// (draws, triangles) per pipeline, in CENSUS_TAGS' order.
pub type DrawCensus = [[f64; 2]; CENSUS_LEN];

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuSample {
    /// Lags a frame or two; 0 before the renderer exists or without GPU timing.
    pub ms: f64,
    pub spans: [f64; GPU_SPANS],
    pub draw_calls: f64,
    pub triangles: f64,
    pub census: DrawCensus,
    /// Slot z: triangles of cells at zoom z.
    pub zoom_tris: [f64; ZOOMS],
    pub px: f64,
    /// 1 where the frame did not fit the GPU stream and was not drawn.
    pub skipped: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSample {
    pub buffers: f64,
    pub buffer_mb: f64,
    pub arena_mb: f64,
    pub depth_mb: f64,
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
    /// Pending chunks outside the frustum. They sort last, so starvation shows here first.
    pub offscreen_pending: f64,
    pub oldest_release_ms: f64,
    pub oldest_visible_ms: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MountSample {
    pub added: f64,
    pub removed: f64,
    pub layouts: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PageSample {
    pub total: f64,
    pub by_zoom: [f64; ZOOMS],
    pub poi: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeapSample {
    pub used_mb: f64,
    /// Handed out by the page level, carved runs counted whole.
    pub live_mb: f64,
    /// Of `live_mb`, what was carved into slots, and what is out in slots;
    /// the difference is the carving's overhead.
    pub run_mb: f64,
    pub slot_mb: f64,
    pub range_mb: f64,
    pub refusals: f64,
    pub largest_free_mb: f64,
}

/// The completion ring's peak, and the most submissions a turn held back for
/// lack of room (each answered a frame late).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RingSample {
    pub cq_peak: f64,
    pub held_peak: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkSample {
    pub scans: f64,
    pub bakes: f64,
    /// Largest mesh handed over since the last frame, in boxes, per slot kind.
    pub fine_peak: f64,
    pub coarse_peak: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    pub t: f64,
    /// The host's rAF interval, not the world's step: they differ past
    /// MAX_STEP_S, which is where the frames worth reading are.
    pub dt_ms: f64,
    pub frame_ms: f64,
    /// 1 when the tick took the large settled budget, so a long dt was chosen.
    pub settled: f64,
    /// From the turn's rising edge to the end of the tick: what `budget_ms` is a
    /// quota on. It exceeds the sum of `phases` by the effects and wait before the
    /// frame task.
    pub sched_ms: f64,
    pub budget_ms: f64,
    pub phases: [f64; PHASES.len()],
    pub chunks_ms: f64,
    /// The kernel's whole turn; `dt_ms - task_ms` is everything outside it.
    pub task_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
    pub lod: LodSample,
    pub mounts: MountSample,
    pub pages: PageSample,
    pub heap: HeapSample,
    pub rings: RingSample,
    pub work: WorkSample,
    // Queued plus in-flight requests, per category.
    pub q_terrain: f64,
    pub q_poi: f64,
    pub stale: StaleSample,
    pub cap: CapSample,
    pub turn: TurnSample,
    pub cover: CoverSample,
    pub view: ViewSample,
}

/// The goal's rays cast and nodes visited this turn.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewSample {
    pub rays: f64,
    pub visits: f64,
}

/// Chunks the goal would be at target quality, pieces at the effective one,
/// and top-level columns with nothing resident yet (`unanswered`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CoverSample {
    pub rule: f64,
    pub drawn: f64,
    pub gaps: f64,
}

/// Lane names follow profiler.json.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StaleSample {
    pub hidden: f64,
    pub uncovered: f64,
    pub unready: f64,
    pub slot_waits: f64,
    pub wait_zooms: f64,
    pub chained: f64,
}

/// The turn in parts that add up to `task_ms`, so unmeasured time shows as
/// the difference. Effects went untimed until one took 70 ms of a turn whose
/// tick was 1.5 ms (2026-09-23).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurnSample {
    pub effects_ms: f64,
    pub stage_ms: f64,
    pub step_ms: f64,
    pub step_name: f64,
}

/// What the frame's ceilings left undone: whether one is holding the screen back.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CapSample {
    pub commit_left: f64,
    pub mount_left: f64,
    pub mount_full: f64,
    pub record_refused: f64,
}

/// Kept with the frame before it, since geometry, textures and heap only mean anything as deltas.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorstFrame {
    pub sample: FrameSample,
    pub has_prev: f64,
    pub prev: FrameSample,
}

/// A record laid out as a profiler.json block. Implemented only through
/// `words!`, which checks every field's offset at compile time.
///
/// # Safety
/// The implementor is `#[repr(C)]` and all f64 (or such records), no padding.
pub unsafe trait Words: Sized {}

pub fn put<T: Words>(out: &mut [f64], block: u32, value: &T) {
    // SAFETY: `Words` guarantees plain f64s laid end to end.
    let words = unsafe { core::slice::from_raw_parts(std::ptr::from_ref::<T>(value).cast::<f64>(), size_of::<T>() / 8) };
    let at = block as usize;
    out[at..at + words.len()].copy_from_slice(words);
}

macro_rules! words {
    ($t:ty, $words:expr, $($($field:ident).+ => $at:expr),+ $(,)?) => {
        const _: () = assert!(size_of::<$t>() == $words as usize * 8, concat!(stringify!($t), " is not the size profiler.json lays out"));
        $(const _: () = assert!(offset_of!($t, $($field).+) == $at as usize * 8, concat!(stringify!($t), " puts ", stringify!($($field).+), " off the word profiler.json gives it"));)+
        // SAFETY: the two assertions above are the claim.
        unsafe impl Words for $t {}
    };
}
pub(crate) use words;

words!(FrameSample, FRAME::WORDS,
    t => FRAME::t, dt_ms => FRAME::dt_ms, frame_ms => FRAME::frame_ms, settled => FRAME::settled,
    sched_ms => FRAME::sched_ms, budget_ms => FRAME::budget_ms, phases => FRAME::phases, chunks_ms => FRAME::chunks_ms, task_ms => FRAME::task_ms,
    gpu.ms => FRAME::gpu_ms, gpu.spans => FRAME::gpu_spans, gpu.draw_calls => FRAME::draw_calls, gpu.triangles => FRAME::triangles,
    gpu.census => FRAME::census, gpu.zoom_tris => FRAME::zoom_tris, gpu.px => FRAME::px, gpu.skipped => FRAME::gpu_skipped,
    device.buffers => FRAME::buffers, device.buffer_mb => FRAME::buffer_mb, device.arena_mb => FRAME::arena_mb,
    device.depth_mb => FRAME::tex_depth_mb, device.shadow_mb => FRAME::tex_shadow_mb, device.layers_mb => FRAME::tex_layers_mb,
    lod.chunks => FRAME::lod_chunks, lod.in_frustum => FRAME::lod_in_frustum, lod.pending => FRAME::lod_pending, lod.settling => FRAME::lod_settling,
    lod.offscreen_pending => FRAME::lod_offscreen_pending,
    lod.oldest_release_ms => FRAME::lod_oldest_release_ms, lod.oldest_visible_ms => FRAME::lod_oldest_visible_ms,
    mounts.added => FRAME::mounts_added, mounts.removed => FRAME::mounts_removed, mounts.layouts => FRAME::mounts_layouts,
    pages.total => FRAME::pages_total, pages.by_zoom => FRAME::zoom_pages, pages.poi => FRAME::pages_poi,
    heap.used_mb => FRAME::heap_used_mb, heap.live_mb => FRAME::heap_live_mb, heap.run_mb => FRAME::heap_run_mb,
    heap.slot_mb => FRAME::heap_slot_mb, heap.range_mb => FRAME::heap_range_mb,
    heap.refusals => FRAME::heap_refusals, heap.largest_free_mb => FRAME::heap_largest_free_mb,
    rings.cq_peak => FRAME::ring_cq_peak, rings.held_peak => FRAME::ring_held_peak,
    work.scans => FRAME::work_scans, work.bakes => FRAME::work_bakes, work.fine_peak => FRAME::work_fine_peak, work.coarse_peak => FRAME::work_coarse_peak, q_terrain => FRAME::q_terrain, q_poi => FRAME::q_poi,
    stale.hidden => FRAME::stale_hidden, stale.uncovered => FRAME::stale_uncovered, stale.unready => FRAME::stale_unready,
    stale.slot_waits => FRAME::slot_waits, stale.wait_zooms => FRAME::slot_wait_zooms,
    stale.chained => FRAME::slot_chained,
    cap.commit_left => FRAME::cap_commit_left, cap.mount_left => FRAME::cap_mount_left,
    cap.mount_full => FRAME::cap_mount_full, cap.record_refused => FRAME::cap_record_refused,
    turn.effects_ms => FRAME::turn_effects_ms, turn.stage_ms => FRAME::turn_stage_ms,
    turn.step_ms => FRAME::turn_step_ms, turn.step_name => FRAME::turn_step_name,
    cover.rule => FRAME::cover_rule, cover.drawn => FRAME::cover_drawn, cover.gaps => FRAME::cover_gaps,
    view.rays => FRAME::view_rays, view.visits => FRAME::view_visits,
);

words!(TaskRow, TASK::WORDS,
    category => TASK::category, subject => TASK::subject, priority => TASK::priority, enqueue_ms => TASK::enqueue_ms,
    start_ms => TASK::start_ms, end_ms => TASK::end_ms, outcome => TASK::outcome,
    fail => TASK::fail, detail => TASK::detail,
    at_file => TASK::at_file, at_file_bytes => TASK::at_file_bytes, at_line => TASK::at_line, phase_count => TASK::phase_count, phase_ids => TASK::phase_ids, phase_values => TASK::phase_values,
);

words!(WorstFrame, WORST::WORDS, sample => WORST::sample, has_prev => WORST::has_prev, prev => WORST::prev);

words!(FramePeak, SUMMARY::peak::WORDS,
    present => SUMMARY::peak::present, dt_ms => SUMMARY::peak::dt_ms, sched_ms => SUMMARY::peak::sched_ms,
    chunks_ms => SUMMARY::peak::chunks_ms, gpu_ms => SUMMARY::peak::gpu_ms, draw_calls => SUMMARY::peak::draw_calls,
    triangles => SUMMARY::peak::triangles, q_terrain => SUMMARY::peak::q_terrain, q_poi => SUMMARY::peak::q_poi,
    chunks => SUMMARY::peak::chunks, in_frustum => SUMMARY::peak::in_frustum, pending => SUMMARY::peak::pending,
    settling => SUMMARY::peak::settling,
    offscreen_pending => SUMMARY::peak::offscreen_pending, oldest_release_ms => SUMMARY::peak::oldest_release_ms,
    oldest_visible_ms => SUMMARY::peak::oldest_visible_ms, fine_peak => SUMMARY::peak::fine_peak,
    coarse_peak => SUMMARY::peak::coarse_peak,
);

const _: () = assert!(PHASE_NAMES.len() == PHASES.len(), "profiler.json names the tick's phases");

pub const GPU_SPANS: usize = GPU_SPAN_NAMES.len();
pub const ZOOMS: usize = LIMITS::ZOOMS as usize;

pub const MB: f64 = 1_048_576.0;
const _: () = assert!(GPU_SPANS == pxl_render::gpu::renderer::GPU_SPANS.len(), "profiler.json names the renderer's spans");

#[cfg(test)]
#[path = "record.test.rs"]
mod test;
const _: () = assert!(
    zoom_of(OBJECT_FINEST) < ZOOMS as i32 && zoom_of(GROUND.scale(TOP_LEVEL)) >= 0,
    "a zoom slot for every cell drawn, the finest object tier to the pyramid's top"
);

/// The rings keep the last minute. At 60 Hz that is 3,600 frames; a faster
/// display or a heavy minute fills a lane early, which drops its oldest and
/// sets `truncated`.
pub const WINDOW_MS: f64 = 60_000.0;
pub const TASK_LIMIT: usize = 2_048;
pub const FRAME_LIMIT: usize = 4_096;
const WORST_CAP: usize = 12;
pub const SPREAD_LIMIT: usize = if FRAME_LIMIT > TASK_LIMIT { FRAME_LIMIT } else { TASK_LIMIT };

/// Advances `head` past records older than `cutoff`; returns how many to drop
/// now, only once the dropped part is the larger half, so most trims are a
/// head step rather than a shift.
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

/// Returns false where a full lane dropped its oldest quarter for `rec`: a
/// shift a quarter of the lane apart rather than one a record.
fn land_lane<T: Copy>(ring: &mut [T], len: &mut usize, head: &mut usize, rec: T) -> bool {
    let full = *len == ring.len() && *head == 0;
    if *len == ring.len() {
        let n = if full { ring.len() / 4 } else { core::mem::take(head) };
        drop_front(ring, len, n);
    }
    ring[*len] = rec;
    *len += 1;
    !full
}

pub(super) fn drop_front<T: Copy>(ring: &mut [T], len: &mut usize, n: usize) {
    ring.copy_within(n..*len, 0);
    *len -= n;
}

#[derive(Clone, Copy)]
pub(super) enum Lane {
    Tasks,
    Frames,
    Worst,
    Scratch,
    LogTasks,
    LogFrames,
}

const LANES: [u32; 6] = [
    (TASK_LIMIT * size_of::<TaskRow>()) as u32,
    (FRAME_LIMIT * size_of::<FrameSample>()) as u32,
    (WORST_CAP * size_of::<WorstFrame>()) as u32,
    (SPREAD_LIMIT * size_of::<f64>()) as u32,
    (PENDING * size_of::<TaskRow>()) as u32,
    (PENDING * size_of::<FrameSample>()) as u32,
];

const LANE_ALIGN: u32 = 64;

pub const PROFILER_BYTES: u32 = span_of(&LANES, LANE_ALIGN);

pub(super) fn fixed_lane<T: Copy>(which: Lane, cap: usize) -> &'static mut [T] {
    // SAFETY: the lane's own bytes, held only by its ring, zeroed when laid;
    // zero is a valid record.
    unsafe { lane(range(Range::Profiler).nth(&LANES, LANE_ALIGN, which as usize), 0, cap) }
}

#[derive(Clone, Copy, Default)]
pub struct ProfilerSink {
    pub task: Option<fn(&TaskRow)>,
    pub frame: Option<fn(&FrameSample)>,
}

/// Worst values since recording started, kept past the ring's minute.
/// Written through `words!`: a hand-ordered copy once shifted every lane
/// from the fifth on by one (2026-09-23).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePeak {
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
    pub offscreen_pending: f64,
    pub oldest_release_ms: f64,
    pub oldest_visible_ms: f64,
    pub fine_peak: f64,
    pub coarse_peak: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecordingState {
    pub recording: bool,
    pub elapsed_ms: f64,
    pub tasks: usize,
    pub frames: usize,
    pub truncated: bool,
}

struct Profiler {
    task_ring: &'static mut [TaskRow],
    task_len: usize,
    frame_ring: &'static mut [FrameSample],
    frame_len: usize,
    task_head: usize,
    frame_head: usize,
    truncated: bool,
    sink: ProfilerSink,
    /// Off until asked, so a client without the dev overlay writes nothing.
    /// Stopping freezes the data for a stable export.
    recording: bool,
    started_at: f64,
    stopped_at: Option<f64>,
    frame_peak: FramePeak,
    worst: &'static mut [WorstFrame],
    worst_len: usize,
    rings: RingSample,
    /// The frame whose task and the gap after it are not known yet.
    open: Option<FrameSample>,
}

static PROFILER: Global<Profiler> = Global::new();

fn profiler() -> &'static mut Profiler {
    PROFILER.get(|| Profiler {
        task_ring: &mut [],
        task_len: 0,
        frame_ring: &mut [],
        frame_len: 0,
        task_head: 0,
        frame_head: 0,
        truncated: false,
        sink: ProfilerSink::default(),
        recording: false,
        started_at: now(),
        stopped_at: None,
        frame_peak: FramePeak::default(),
        worst: &mut [],
        worst_len: 0,
        rings: RingSample::default(),
        open: None,
    })
}

pub fn profiler_set_sink(sink: Option<ProfilerSink>) {
    profiler().sink = sink.unwrap_or_default();
}

pub fn profiler_recording() -> bool {
    profiler().recording
}

pub fn profiler_recording_state() -> RecordingState {
    let l = profiler();
    RecordingState {
        recording: l.recording,
        elapsed_ms: l.stopped_at.unwrap_or_else(now) - l.started_at,
        tasks: l.task_len - l.task_head,
        frames: l.frame_len - l.frame_head,
        truncated: l.truncated,
    }
}

pub fn profiler_recording_start() {
    let l = profiler();
    (l.task_len, l.frame_len, l.worst_len) = (0, 0, 0);
    (l.task_head, l.frame_head) = (0, 0);
    l.frame_peak = FramePeak::default();
    l.open = None;
    l.truncated = false;
    l.started_at = now();
    l.stopped_at = None;
    l.recording = true;
}

pub fn profiler_recording_stop() {
    let l = profiler();
    if !l.recording {
        return;
    }
    l.recording = false;
    l.stopped_at = Some(now());
}

pub fn profiler_record(rec: TaskRecord) {
    let l = profiler();
    if !l.recording {
        return;
    }
    let row = TaskRow::of(&rec);
    if let Some(task) = l.sink.task {
        task(&row);
    }
    if l.task_ring.is_empty() {
        l.task_ring = fixed_lane(Lane::Tasks, TASK_LIMIT);
    }
    let dropped = trim(&l.task_ring[..l.task_len], &mut l.task_head, row.end_ms - WINDOW_MS, |r| r.end_ms);
    drop_front(l.task_ring, &mut l.task_len, dropped);
    if !land_lane(l.task_ring, &mut l.task_len, &mut l.task_head, row) {
        l.truncated = true;
    }
}

pub fn profiler_frame_peak() -> FramePeak {
    profiler().frame_peak
}

pub fn profiler_note_rings(cq_len: u32, held: u32) {
    let r = &mut profiler().rings;
    r.cq_peak = r.cq_peak.max(cq_len as f64);
    r.held_peak = r.held_peak.max(held as f64);
}

pub fn profiler_take_rings() -> RingSample {
    core::mem::take(&mut profiler().rings)
}

/// Lands the open frame with the gap that ended at `s`, which its turn made,
/// and opens `s`.
pub fn profiler_record_frame(s: FrameSample) {
    let l = profiler();
    if !l.recording {
        return;
    }
    let Some(mut f) = l.open.replace(s) else { return };
    (f.dt_ms, f.settled) = (s.dt_ms, s.settled);
    let s = f;
    if l.worst.is_empty() {
        l.worst = fixed_lane(Lane::Worst, WORST_CAP);
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

    if let Some(frame) = l.sink.frame {
        frame(&s);
    }
    if l.frame_ring.is_empty() {
        l.frame_ring = fixed_lane(Lane::Frames, FRAME_LIMIT);
    }
    let dropped = trim(&l.frame_ring[..l.frame_len], &mut l.frame_head, s.t - WINDOW_MS, |f| f.t);
    drop_front(l.frame_ring, &mut l.frame_len, dropped);
    if !land_lane(l.frame_ring, &mut l.frame_len, &mut l.frame_head, s) {
        l.truncated = true;
    }
}

/// The open frame, so `task_ms`, known only after the turn, lands in it.
/// None once recording stopped.
pub fn profiler_open_frame() -> Option<&'static mut FrameSample> {
    let l = profiler();
    if !l.recording {
        return None;
    }
    l.open.as_mut()
}

fn first_at<T>(ring: &[T], t: f64, at: impl Fn(&T) -> f64) -> usize {
    ring.partition_point(|rec| at(rec) < t)
}

pub fn profiler_frames() -> &'static [FrameSample] {
    let l = profiler();
    &l.frame_ring[l.frame_head..l.frame_len]
}

/// In `end_ms` order, since a task is recorded as it settles.
pub fn profiler_tasks() -> &'static [TaskRow] {
    let l = profiler();
    &l.task_ring[l.task_head..l.task_len]
}

pub fn profiler_worst() -> &'static [WorstFrame] {
    let l = profiler();
    &l.worst[..l.worst_len]
}

pub fn profiler_frames_since(t: f64) -> &'static [FrameSample] {
    let ring = profiler_frames();
    &ring[first_at(ring, t, |f| f.t)..]
}

pub fn profiler_tasks_since(t: f64) -> &'static [TaskRow] {
    let ring = profiler_tasks();
    &ring[first_at(ring, t, |r| r.end_ms)..]
}
