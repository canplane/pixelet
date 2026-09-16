// pxl_core/module/world/sched/ledger.rs

//! Read-only instrument for the schedulers: records where time and work go so
//! it can be seen. Never read back into a scheduling decision — the
//! observation→execution edge is one-directional on purpose
//! (issue/20260909-cooperative-scheduler-and-task-ledger.md).
//!
//! Two record types, one instrument:
//! - TaskRecord — one finished unit of deferred work (a fetch_scheduler.rs
//!   demand today; a CPU task later). `transport_ms` is 0 while a task body
//!   runs inline, and holds the transfer cost once it runs in a worker.
//! - FrameSample — one rendered frame's gauges (pacing, GPU ask, memory, queue
//!   pressure). Only accrues while frames actually render (the tick runs only
//!   when asked), which is correct: idle means nothing to sample.
//!
//! Readers: the overlay and the text report (log.rs), both through
//! summary.rs, and the Chrome-trace export (trace.rs).

use core::fmt;

use pxl_shared::spec::fetch::FetchFail;

use super::demand::{DemandKey, Phases};
use super::tick::PHASES;
use crate::global::{now, Global};
use crate::kernel::kernel;
use crate::memory::FIXED_POOL;

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

#[derive(Clone, Debug, PartialEq)]
pub struct TaskRecord {
    pub key: DemandKey,
    pub category: &'static str,
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

/// (draws, triangles) by the pipeline a draw names (render/draw.rs), for one
/// frame's render.
pub type DrawCensus = [(f64, f64); CENSUS_TAGS.len()];

/// A census entry's tag: the pipeline at its index.
pub const CENSUS_TAGS: [&str; 10] = ["chunk", "decor", "ghost_box", "ghost_coplanar", "ghost_decor", "skinned", "puck_zone", "puck_beacon", "badge", "debug"];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CpuSample {
    /// The kernel's tick, inside the visible phase.
    pub tick_ms: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TexturesMb {
    pub depth: f64,
    pub msaa: f64,
    pub shadow: f64,
    pub layers: f64,
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MountSample {
    pub added: f64,
    pub removed: f64,
    pub layouts: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolSample {
    pub live_mb: f64,
    pub idle_mb: f64,
    pub hits: f64,
    pub misses: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkSample {
    pub scans: f64,
    pub bakes: f64,
}

/// One rendered frame. Lean on purpose — latency percentiles, decode bandwidth
/// and upload cost are derived from TaskRecords or added later. Plain numbers,
/// so the ring holds it in place.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    /// monotonic ms at record time.
    pub t: f64,
    /// rAF interval in ms — pacing, read against `frame_ms`.
    pub dt_ms: f64,
    /// The display's measured frame interval when this frame was scheduled
    /// (sched/tick.rs) — what `dt_ms` has to beat.
    pub frame_ms: f64,
    /// The view had held still long enough that the tick took the large,
    /// throughput budget. A long dt on such a frame was chosen, not suffered.
    pub settled: bool,
    /// The whole tick, every phase.
    pub sched_ms: f64,
    /// What the tick was allowed this frame — derived, so it moves.
    pub budget_ms: f64,
    /// sched_ms broken down by phase, in the tick's own order, as PHASES names them.
    pub phases: [f64; PHASES.len()],
    pub cpu: CpuSample,
    /// The whole rAF task: from the scheduler pass starting to the JS stack
    /// emptying, so it covers every phase of the tick *and* the renderer's
    /// encode and submit. `dt_ms - task_ms` is then everything outside the main
    /// thread's own task — vsync wait, compositor, GPU, GC between frames.
    pub task_ms: f64,
    /// From a submit to the queue reporting the work done: lags a frame or two.
    /// 0 before the renderer exists.
    pub gpu_ms: f64,
    pub draw_calls: f64,
    pub triangles: f64,
    /// GPU buffers held, and what they hold in MiB — their deltas across frames
    /// are the upload churn.
    pub buffers: f64,
    pub buffer_mb: f64,
    pub textures_mb: TexturesMb,
    pub census: DrawCensus,
    /// The canvas's width*height — the raw fill-rate primitive.
    pub px: f64,
    pub lod: LodSample,
    pub mounts: MountSample,
    /// used JS heap in MiB — Chrome only, 0 where unavailable.
    pub heap_mb: f64,
    pub tile_pages: f64,
    pub poi_count: f64,
    pub pool: PoolSample,
    pub work: WorkSample,
    // network pressure — fetch_scheduler queued + in-flight, per category
    pub q_terrain: f64,
    pub q_poi: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongFrameSource {
    Loaf,
    Longtask,
}

/// One long animation frame (LoAF, >50ms). Carries LoAF's own attribution so a
/// stutter pins to script (CPU) vs render/style-layout without guessing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LongFrameRecord {
    /// which observer produced it — LoAF is richer but unsupported outside
    /// Chromium, so `longtask` is the fallback that actually fires elsewhere.
    pub source: LongFrameSource,
    pub t: f64,
    pub duration_ms: f64,
    /// time past 50ms the frame blocked the event loop.
    pub blocking_ms: f64,
    /// summed <script> entry durations — the CPU half.
    pub script_ms: f64,
    /// the render/style/layout half; 0 if the browser didn't report renderStart.
    pub render_ms: f64,
}

/// The rings keep the last minute: a record older than WINDOW_MS is dropped as
/// a new one lands, so what the ledger holds is bounded by the display's rate,
/// not by how long the tab has been open. A report or a trace covers the minute
/// before it was taken. The rings are lanes of the fixed section at the
/// capacities below — a minute at 60 Hz is 3,600 frames, about 2 MB of samples
/// — so a faster display, or a minute of more tasks than a pan asks for, fills
/// its lane before the minute passes; filling one sets `truncated`.
pub const WINDOW_MS: f64 = 60_000.0;
const TASK_LIMIT: usize = 2_048;
const FRAME_LIMIT: usize = 4_096;
const LONG_FRAME_LIMIT: usize = 1_024;

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

/// `rec` onto a lane ring past its trim; when the lane is full the records the
/// window already dropped make room first, and past that the ring is truncated.
fn land<T>(ring: &mut Vec<T>, cap: usize, head: &mut usize, rec: T) -> bool {
    if ring.len() == cap && *head > 0 {
        ring.drain(..core::mem::take(head));
    }
    if ring.len() == cap {
        return false;
    }
    ring.push(rec);
    true
}

/// The same over a lane of the fixed section holding `len` records.
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
fn drop_front<T: Copy>(ring: &mut [T], len: &mut usize, n: usize) {
    ring.copy_within(n..*len, 0);
    *len -= n;
}

/// A lane of the fixed section for `cap` records, zeroed; none where there is
/// no section (a native test), and then nothing is recorded into it. A minute
/// of frame samples is megabytes, which the module heap is not for.
fn fixed_lane<T: Copy>(cap: usize) -> &'static mut [T] {
    let bytes = cap * core::mem::size_of::<T>();
    let Some(at) = kernel().pools.get_mut(FIXED_POOL as usize).and_then(|p| p.acquire(bytes as u32)) else { return &mut [] };
    // SAFETY: a block of the fixed section this lane holds alone and never
    // gives back, `bytes` long and aligned past a record's alignment; zeroed,
    // which is a valid record for the plain numbers, flags and field-less
    // enums the rings hold.
    unsafe {
        core::ptr::write_bytes(at as *mut u8, 0, bytes);
        core::slice::from_raw_parts_mut(at as *mut T, cap)
    }
}

/// Live consumer for records as they land — log.rs's console stream. One sink,
/// not a list: the instrument has a single output mode at a time.
#[derive(Clone, Copy, Default)]
pub struct LedgerSink {
    pub task: Option<fn(&TaskRecord)>,
    pub frame: Option<fn(&FrameSample)>,
    pub long_frame: Option<fn(&LongFrameRecord)>,
}

/// The worst-seen values since recording started, kept as frames land so the
/// report never scans the ring for them — and past the minute the ring keeps,
/// so a stall noticed late is still on record. Reset with the recording.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePeak {
    pub dt_ms: f64,
    pub sched_ms: f64,
    pub tick_ms: f64,
    pub draw_calls: f64,
    pub triangles: f64,
    pub heap_mb: f64,
    pub q_terrain: f64,
    pub q_poi: f64,
    pub chunks: f64,
    pub in_frustum: f64,
    pub pending: f64,
    pub offscreen_pending: f64,
    pub oldest_release_ms: f64,
    pub oldest_visible_ms: f64,
    pub settling: f64,
    pub blend_active: f64,
    pub pressure: f64,
}

/// The worst frames by dt since recording started, kept whole with the frame
/// before each, because geometry/texture/heap only mean anything as deltas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorstFrame {
    pub sample: FrameSample,
    pub prev: Option<FrameSample>,
}

const WORST_CAP: usize = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecordingState {
    pub recording: bool,
    /// ms of wall time the window spans; live while recording.
    pub elapsed_ms: f64,
    pub tasks: usize,
    pub frames: usize,
    pub long_frames: usize,
    pub truncated: bool,
}

struct Ledger {
    /// The tasks on the module heap: a record carries its key and its error
    /// as strings.
    task_ring: Vec<TaskRecord>,
    /// The frames and the long frames in lanes of the fixed section, taken at
    /// their first record, `*_len` of each in use.
    frame_ring: &'static mut [FrameSample],
    frame_len: usize,
    long_frame_ring: &'static mut [LongFrameRecord],
    long_frame_len: usize,
    /// Where each ring's live window starts: records before it are past the
    /// minute and wait for `trim` to drop them.
    task_head: usize,
    frame_head: usize,
    long_frame_head: usize,
    truncated: bool,
    sink: LedgerSink,
    /// Explicit recording window: stopping freezes the data so an export is
    /// stable while it is being read; restarting clears and begins a fresh one.
    recording: bool,
    started_at: f64,
    stopped_at: Option<f64>,
    frame_peak: FramePeak,
    /// Slowest first.
    worst_frames: Vec<WorstFrame>,
}

static LEDGER: Global<Ledger> = Global::new();

fn ledger() -> &'static mut Ledger {
    LEDGER.get(|| Ledger {
        task_ring: Vec::with_capacity(TASK_LIMIT),
        frame_ring: &mut [],
        frame_len: 0,
        long_frame_ring: &mut [],
        long_frame_len: 0,
        task_head: 0,
        frame_head: 0,
        long_frame_head: 0,
        truncated: false,
        sink: LedgerSink::default(),
        recording: true,
        started_at: now(),
        stopped_at: None,
        frame_peak: FramePeak::default(),
        worst_frames: Vec::with_capacity(WORST_CAP),
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
        tasks: l.task_ring.len() - l.task_head,
        frames: l.frame_len - l.frame_head,
        long_frames: l.long_frame_len - l.long_frame_head,
        truncated: l.truncated,
    }
}

/// Clears everything and opens a fresh window.
pub fn ledger_recording_start() {
    let l = ledger();
    l.task_ring.clear();
    (l.frame_len, l.long_frame_len) = (0, 0);
    (l.task_head, l.frame_head, l.long_frame_head) = (0, 0, 0);
    l.worst_frames.clear();
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
    if let Some(task) = l.sink.task {
        task(&rec);
    }
    let dropped = trim(l.task_ring.as_slice(), &mut l.task_head, rec.end_ms - WINDOW_MS, |r| r.end_ms);
    l.task_ring.drain(..dropped);
    if !land(&mut l.task_ring, TASK_LIMIT, &mut l.task_head, rec) {
        l.truncated = true;
    }
}

/// Copy of the window's worst-seen frame values.
pub fn ledger_frame_peak() -> FramePeak {
    ledger().frame_peak
}

/// Window worst frames, slowest first.
pub fn ledger_worst_frames() -> &'static [WorstFrame] {
    ledger().worst_frames.as_slice()
}

pub fn ledger_record_frame(s: FrameSample) {
    let l = ledger();
    if !l.recording {
        return;
    }
    let worst = &mut l.worst_frames;
    let at = worst.as_slice().partition_point(|w| w.sample.dt_ms >= s.dt_ms);
    if at < WORST_CAP {
        worst.truncate(WORST_CAP - 1);
        worst.insert(
            at.min(worst.len()),
            WorstFrame {
                sample: s,
                prev: l.frame_ring[..l.frame_len].last().copied(),
            },
        );
    }

    let p = &mut l.frame_peak;
    p.dt_ms = p.dt_ms.max(s.dt_ms);
    p.sched_ms = p.sched_ms.max(s.sched_ms);
    p.tick_ms = p.tick_ms.max(s.cpu.tick_ms);
    p.draw_calls = p.draw_calls.max(s.draw_calls);
    p.triangles = p.triangles.max(s.triangles);
    p.heap_mb = p.heap_mb.max(s.heap_mb);
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

/// A long frame the shell's PerformanceObserver saw (LoAF, or longtask where
/// LoAF is missing — both feed one ring and each record says which).
pub fn ledger_record_long_frame(rec: LongFrameRecord) {
    let l = ledger();
    if !l.recording {
        return;
    }
    if l.long_frame_ring.is_empty() {
        l.long_frame_ring = fixed_lane(LONG_FRAME_LIMIT);
    }
    let dropped = trim(&l.long_frame_ring[..l.long_frame_len], &mut l.long_frame_head, rec.t - WINDOW_MS, |r| r.t);
    drop_front(l.long_frame_ring, &mut l.long_frame_len, dropped);
    if !land_lane(l.long_frame_ring, &mut l.long_frame_len, &mut l.long_frame_head, rec) {
        l.truncated = true;
        return;
    }
    if let Some(long_frame) = l.sink.long_frame {
        long_frame(&rec);
    }
}

/// The task window, newest last.
pub fn ledger_snapshot() -> &'static [TaskRecord] {
    let l = ledger();
    &l.task_ring.as_slice()[l.task_head..]
}

/// The frame sample just recorded, so a late-arriving measurement (task_ms,
/// which is only known once the rAF task's stack empties) can be written into
/// the frame it belongs to instead of being attributed to the next one.
pub fn ledger_last_frame() -> Option<&'static mut FrameSample> {
    let l = ledger();
    l.frame_ring[..l.frame_len].last_mut()
}

/// First index in a ring, ordered by `at`, whose record is at or after `t`.
fn first_at<T>(ring: &[T], t: f64, at: impl Fn(&T) -> f64) -> usize {
    ring.partition_point(|rec| at(rec) < t)
}

/// Frames recorded at or after `t`, newest last — a live reader's window, so
/// it costs the tail it reads and not the whole recording.
pub fn ledger_frames_since(t: f64) -> &'static [FrameSample] {
    let l = ledger();
    let ring = &l.frame_ring[l.frame_head..l.frame_len];
    &ring[first_at(ring, t, |f| f.t)..]
}

/// Tasks settled at or after `t`. A task is recorded as it settles, so the
/// ring is in `end_ms` order.
pub fn ledger_tasks_since(t: f64) -> &'static [TaskRecord] {
    let l = ledger();
    let ring = &l.task_ring.as_slice()[l.task_head..];
    &ring[first_at(ring, t, |r| r.end_ms)..]
}

/// Long frames starting at or after `t`. Filtered rather than searched: two
/// observers feed this ring, so it is not strictly in time order, and it is
/// short.
pub fn ledger_long_frames_since(t: f64) -> Vec<LongFrameRecord> {
    let l = ledger();
    l.long_frame_ring[l.long_frame_head..l.long_frame_len].iter().filter(|r| r.t >= t).copied().collect()
}

/// The frame window, newest last.
pub fn ledger_frames_snapshot() -> &'static [FrameSample] {
    let l = ledger();
    &l.frame_ring[l.frame_head..l.frame_len]
}

/// The long-frame window, newest last.
pub fn ledger_long_frames_snapshot() -> &'static [LongFrameRecord] {
    let l = ledger();
    &l.long_frame_ring[l.long_frame_head..l.long_frame_len]
}
