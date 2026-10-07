// pxl-kernel/sched/profiler/record.rs

//! Read-only instrument: records where time and work go. Never read back
//! into a scheduling decision. Records are laid out word for word as
//! profiler.json says, in lanes the shell reads in place, so reading the
//! profiler allocates nothing in the heap it measures.

use core::mem::size_of;

use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::mem::section::span_of;

use crate::mem::fixed::{lane, range, Range};
use crate::sched::profiler::frame::{FramePeak, FrameSample, RingSample, WorstFrame};
use crate::sched::profiler::log::PENDING;
use crate::sched::profiler::task::{TaskRecord, TaskRow};

/// The rings keep the last minute. At 60 Hz that is 3,600 frames; a faster
/// display or a heavy minute fills a lane early, which drops its oldest and
/// sets `truncated`.
const WINDOW_MS: f64 = 60_000.0;
const TASK_LIMIT: usize = 2_048;
const FRAME_LIMIT: usize = 4_096;
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

#[cfg(test)]
#[path = "record.test.rs"]
mod test;
