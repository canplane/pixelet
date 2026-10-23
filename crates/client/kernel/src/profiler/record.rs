// pxl-kernel/profiler/record.rs

//! Read-only instrument: records where time and work go. Never read back
//! into a scheduling decision. Records are laid out word for word as
//! profiler.json says, in lanes the shell reads in place, so reading the
//! profiler allocates nothing in the heap it measures.

use core::mem::size_of;

use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::mem::section::span_of;

use crate::mem::fixed::{lane, Held};
use crate::profiler::block::RecBlock;
use crate::profiler::frame::{FrameSample, RingSample, WorstFrame};
use crate::profiler::log::PENDING;
use crate::profiler::task::TaskRow;

/// `.fixed.profiler`, handed in at boot.
pub static RANGE: Held = Held::new();

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
pub(crate) enum Lane {
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

pub(crate) fn fixed_lane<T: Copy>(which: Lane, cap: usize) -> &'static mut [T] {
    // SAFETY: the lane's own bytes, held only by its ring, zeroed when laid;
    // zero is a valid record.
    unsafe { lane(RANGE.get().nth(&LANES, LANE_ALIGN, which as usize), 0, cap) }
}

#[derive(Clone, Copy, Default)]
pub struct ProfilerSink {
    pub task: Option<fn(&TaskRow)>,
    pub frame: Option<fn(&FrameSample)>,
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
    worst: &'static mut [WorstFrame],
    worst_len: usize,
    rings: RingSample,
    /// The interval in progress: the last frame turn's record, which every
    /// turn after it adds its time to until the next frame turn closes it.
    open: Option<FrameSample>,
    /// This turn's frame task's time, which the turn's end takes out of the
    /// time its tasks ran.
    frame_task_ms: f64,
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
        worst: &mut [],
        worst_len: 0,
        rings: RingSample::default(),
        open: None,
        frame_task_ms: 0.0,
    })
}

pub fn profiler_set_sink(sink: Option<ProfilerSink>) {
    profiler().sink = sink.unwrap_or_default();
}

pub fn profiler_recording() -> bool {
    profiler().recording
}

/// The recording as the summary's block holds it, each flag 0 or 1.
pub fn profiler_recording_state() -> RecBlock {
    let l = profiler();
    RecBlock {
        recording: l.recording as u8 as f64,
        elapsed_ms: l.stopped_at.unwrap_or_else(now) - l.started_at,
        truncated: l.truncated as u8 as f64,
    }
}

pub fn profiler_recording_start() {
    let l = profiler();
    (l.task_len, l.frame_len, l.worst_len) = (0, 0, 0);
    (l.task_head, l.frame_head) = (0, 0);
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

/// `row` is made only while recording.
pub fn profiler_record(row: impl FnOnce() -> TaskRow) {
    let l = profiler();
    if !l.recording {
        return;
    }
    let row = row();
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

pub fn profiler_note_rings(cq_len: u32, held: u32) {
    let r = &mut profiler().rings;
    r.cq_peak = r.cq_peak.max(cq_len as f64);
    r.held_peak = r.held_peak.max(held as f64);
}

pub fn profiler_take_rings() -> RingSample {
    core::mem::take(&mut profiler().rings)
}

/// A frame turn, `s` its record: closes the interval in progress with
/// `interval_ms`, the host's gap that just ended, and `settled`, whether the
/// host settled it, lands it, and opens `s`'s. `frame_task_ms` is how long
/// the frame task ran, the turn's end's to take out of the tasks' time.
pub fn profiler_record_frame(s: FrameSample, interval_ms: f64, settled: bool, frame_task_ms: f64) {
    let l = profiler();
    if !l.recording {
        return;
    }
    l.frame_task_ms = frame_task_ms;
    let Some(mut f) = l.open.replace(s) else { return };
    (f.interval_ms, f.settled) = (interval_ms, f64::from(u8::from(settled)));
    land(l, f);
}

/// A turn's end, frame turn or not: its time into the interval in progress,
/// rise to fall, its tasks' part, and what it asked as it fell.
pub fn profiler_record_turn(turn_ms: f64, poll_ms: f64, wake: f64, wake_in_ms: f64) {
    let l = profiler();
    let frame_task_ms = core::mem::take(&mut l.frame_task_ms);
    let Some(f) = l.open.as_mut().filter(|_| l.recording) else { return };
    f.turns += 1.0;
    f.busy_ms += turn_ms;
    // The frame task runs inside the poll, so this is never negative.
    f.tasks_ms += poll_ms - frame_task_ms;
    (f.wake, f.wake_in_ms) = (wake, wake_in_ms);
}

fn land(l: &mut Profiler, s: FrameSample) {
    if l.worst.is_empty() {
        l.worst = fixed_lane(Lane::Worst, WORST_CAP);
    }
    let at = l.worst[..l.worst_len].partition_point(|w| w.sample.interval_ms >= s.interval_ms);
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
