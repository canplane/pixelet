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

use super::demand::Phases;
use crate::global::{now, Global};

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
    pub key: String,
    pub category: String,
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
    /// Why it failed, when `outcome` is Error: the HTTP status where there was
    /// one, otherwise the error's name and the start of its message. The count
    /// alone could not say whether twelve failures were one cause or twelve.
    pub error: Option<String>,
}

/// tag → (draws, triangles), for one frame's render.
pub type DrawCensus = Vec<(String, (f64, f64))>;

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
/// and upload cost are derived from TaskRecords or added later.
#[derive(Clone, Debug, Default, PartialEq)]
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
    /// sched_ms broken down by phase, in the tick's own order.
    pub phases: Vec<(String, f64)>,
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

/// Recording is **exhaustive for the session** — every task and every rendered
/// frame, from load until the tab goes away. It was a rolling window and that
/// was simply wrong: the one thing this instrument exists to capture is a
/// stall, and a stall had already been overwritten by the time anyone pulled a
/// report or a trace. The caps below are pure safety valves against an
/// unattended tab left running for hours, not a retention policy — hitting one
/// sets `truncated`, so an export can say so.
const TASK_LIMIT: usize = 200_000;
const FRAME_LIMIT: usize = 500_000; // ~2.3 hours at 60fps
const LONG_FRAME_LIMIT: usize = 50_000;

/// Live consumer for records as they land — log.rs's console stream. One sink,
/// not a list: the instrument has a single output mode at a time.
#[derive(Clone, Copy, Default)]
pub struct LedgerSink {
    pub task: Option<fn(&TaskRecord)>,
    pub frame: Option<fn(&FrameSample)>,
    pub long_frame: Option<fn(&LongFrameRecord)>,
}

/// The window's worst-seen values, kept as frames land so the report never
/// scans the ring for them. Reset with the window.
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

/// The window's worst frames by dt, kept whole with the frame before each,
/// because geometry/texture/heap only mean anything as deltas.
#[derive(Clone, Debug, PartialEq)]
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
    task_ring: Vec<TaskRecord>,
    frame_ring: Vec<FrameSample>,
    long_frame_ring: Vec<LongFrameRecord>,
    truncated: bool,
    sink: LedgerSink,
    /// Explicit recording window: stopping freezes the data so an export is
    /// stable while it is being read; restarting clears and begins a fresh one.
    recording: bool,
    started_at: f64,
    stopped_at: Option<f64>,
    frame_peak: FramePeak,
    worst_frames: Vec<WorstFrame>,
}

static LEDGER: Global<Ledger> = Global::new();

fn ledger() -> &'static mut Ledger {
    LEDGER.get(|| Ledger {
        task_ring: Vec::new(),
        frame_ring: Vec::new(),
        long_frame_ring: Vec::new(),
        truncated: false,
        sink: LedgerSink::default(),
        recording: true,
        started_at: now(),
        stopped_at: None,
        frame_peak: FramePeak::default(),
        worst_frames: Vec::new(),
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
        tasks: l.task_ring.len(),
        frames: l.frame_ring.len(),
        long_frames: l.long_frame_ring.len(),
        truncated: l.truncated,
    }
}

/// Clears everything and opens a fresh window.
pub fn ledger_recording_start() {
    let l = ledger();
    l.task_ring.clear();
    l.frame_ring.clear();
    l.long_frame_ring.clear();
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
    if l.task_ring.len() >= TASK_LIMIT {
        l.truncated = true;
    } else {
        l.task_ring.push(rec);
    }
}

/// Copy of the window's worst-seen frame values.
pub fn ledger_frame_peak() -> FramePeak {
    ledger().frame_peak
}

/// Window worst frames, slowest first.
pub fn ledger_worst_frames() -> Vec<WorstFrame> {
    ledger().worst_frames.clone()
}

pub fn ledger_record_frame(s: FrameSample) {
    let l = ledger();
    if !l.recording {
        return;
    }
    let prev = l.frame_ring.last().cloned();
    if l.worst_frames.len() < WORST_CAP || l.worst_frames.last().is_some_and(|w| s.dt_ms > w.sample.dt_ms) {
        l.worst_frames.push(WorstFrame { sample: s.clone(), prev });
        l.worst_frames.sort_by(|a, b| b.sample.dt_ms.total_cmp(&a.sample.dt_ms));
        l.worst_frames.truncate(WORST_CAP);
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
    if l.frame_ring.len() >= FRAME_LIMIT {
        l.truncated = true;
    } else {
        l.frame_ring.push(s);
    }
}

/// A long frame the shell's PerformanceObserver saw (LoAF, or longtask where
/// LoAF is missing — both feed one ring and each record says which).
pub fn ledger_record_long_frame(rec: LongFrameRecord) {
    let l = ledger();
    if !l.recording {
        return;
    }
    if l.long_frame_ring.len() >= LONG_FRAME_LIMIT {
        l.truncated = true;
        return;
    }
    l.long_frame_ring.push(rec);
    if let Some(long_frame) = l.sink.long_frame {
        long_frame(&rec);
    }
}

/// Copy of the task buffer, newest last.
pub fn ledger_snapshot() -> Vec<TaskRecord> {
    ledger().task_ring.clone()
}

/// The frame sample just recorded, so a late-arriving measurement (task_ms,
/// which is only known once the rAF task's stack empties) can be written into
/// the frame it belongs to instead of being attributed to the next one.
pub fn ledger_last_frame() -> Option<&'static mut FrameSample> {
    ledger().frame_ring.last_mut()
}

/// First index in a ring, ordered by `at`, whose record is at or after `t`.
fn first_at<T>(ring: &[T], t: f64, at: impl Fn(&T) -> f64) -> usize {
    ring.partition_point(|rec| at(rec) < t)
}

/// Frames recorded at or after `t`, newest last — a live reader's window, so
/// it costs the tail it reads and not the whole recording.
pub fn ledger_frames_since(t: f64) -> &'static [FrameSample] {
    let ring = &ledger().frame_ring;
    &ring[first_at(ring, t, |f| f.t)..]
}

/// Tasks settled at or after `t`. A task is recorded as it settles, so the
/// ring is in `end_ms` order.
pub fn ledger_tasks_since(t: f64) -> &'static [TaskRecord] {
    let ring = &ledger().task_ring;
    &ring[first_at(ring, t, |r| r.end_ms)..]
}

/// Long frames starting at or after `t`. Filtered rather than searched: two
/// observers feed this ring, so it is not strictly in time order, and it is
/// short.
pub fn ledger_long_frames_since(t: f64) -> Vec<LongFrameRecord> {
    ledger().long_frame_ring.iter().filter(|l| l.t >= t).copied().collect()
}

/// The frame buffer, newest last.
pub fn ledger_frames_snapshot() -> &'static [FrameSample] {
    &ledger().frame_ring
}

/// The long-frame buffer, newest last.
pub fn ledger_long_frames_snapshot() -> &'static [LongFrameRecord] {
    &ledger().long_frame_ring
}
