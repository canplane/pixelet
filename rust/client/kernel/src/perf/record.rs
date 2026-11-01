// pxl-kernel/perf/record.rs

//! Read-only instrument: records where time and work go. Never read back
//! into a scheduling decision. Records are laid out word for word as
//! perf.json says, in columns the shell reads in place, so reading the
//! profiler allocates nothing in the heap it measures. The columns are the
//! heap's while the profiler's setting is on, taken whole as it starts
//! recording and given back as it is turned off (`perf_release`): its
//! heap figures count them all along, so a frame's deltas do not.

use core::mem::size_of;

use xpute_guest::clock::now;
use xpute_guest::global::Global;

use crate::perf::frame::{FrameSample, WorstFrame};
use crate::perf::report::{GpuRead, RayRead};
use crate::perf::section::{RecSection, RecordingSection};
use crate::perf::spec::LIMIT;
use crate::perf::task::TaskRow;

/// The rings keep the last minute (perf.json `LIMIT.WINDOW_MS`, the
/// shell's long frames alike). At 60 Hz that is 3,600 frames; a faster
/// display or a heavy minute fills a column early, which drops its oldest and
/// sets `truncated`.
const WINDOW_MS: f64 = LIMIT::WINDOW_MS as f64;
const TASK_LIMIT: usize = 2_048;
const FRAME_LIMIT: usize = 4_096;
const WORST_CAP: usize = 12;
pub const PERCENTILES_LIMIT: usize = if FRAME_LIMIT > TASK_LIMIT {
    FRAME_LIMIT
} else {
    TASK_LIMIT
};

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

/// Returns false where a full column dropped its oldest quarter for `rec`: a
/// shift a quarter of the column apart rather than one a record.
fn land_column<T: Copy>(ring: &mut [T], len: &mut usize, head: &mut usize, rec: T) -> bool {
    let full = *len == ring.len() && *head == 0;
    if *len == ring.len() {
        let n = if full {
            ring.len() / 4
        } else {
            core::mem::take(head)
        };
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

/// The columns, in order: tasks, frames, the worst frames and the read's
/// scratch (read.rs).
const COLUMNS: [u32; 4] = [
    (TASK_LIMIT * size_of::<TaskRow>()) as u32,
    (FRAME_LIMIT * size_of::<FrameSample>()) as u32,
    (WORST_CAP * size_of::<WorstFrame>()) as u32,
    (PERCENTILES_LIMIT * size_of::<f64>()) as u32,
];

/// What the heap holds of the profiler while its setting is on, in the
/// buddy's blocks: each column a power of two as the heap gives it
/// (door/layout.rs `HEAP_FLOWS_BYTES`).
pub const PERF_BYTES: u64 = {
    let mut sum = 0;
    let mut k = 0;
    while k < COLUMNS.len() {
        sum += (COLUMNS[k] as u64).next_power_of_two();
        k += 1;
    }
    sum
};

/// A column of `n` records, zeroed, a valid record each; none where the heap
/// refuses it.
fn heap_column<T: Copy + Default>(n: usize) -> Option<Box<[T]>> {
    let mut column = Vec::new();
    column.try_reserve_exact(n).ok()?;
    column.resize(n, T::default());
    Some(column.into_boxed_slice())
}

struct Perf {
    task_ring: Box<[TaskRow]>,
    task_len: usize,
    frame_ring: Box<[FrameSample]>,
    frame_len: usize,
    task_head: usize,
    frame_head: usize,
    truncated: bool,
    /// Off until asked, so a client without the profiler on writes nothing.
    /// Stopping freezes the data for a stable export.
    recording: bool,
    started_at: f64,
    stopped_at: Option<f64>,
    worst: Box<[WorstFrame]>,
    worst_len: usize,
    /// What the frames held at the most or the least since the recording
    /// started (`perf_fold_recording`).
    held: RecordingSection,
    /// The read's, where it sorts what it takes percentiles of (read.rs).
    scratch: Box<[f64]>,
    /// The interval in progress: the last frame turn's record, which every
    /// turn after it adds its time to until the next frame turn closes it.
    open: Option<FrameSample>,
    /// This turn's frame task's time, which the turn's end takes out of the
    /// time its tasks ran.
    frame_task_ms: f64,
    /// Whether this turn served the shell's read of the profiler: what it
    /// asked as it fell is no wake of the world's.
    read_turn: bool,
}

static PERF: Global<Perf> = Global::new();

fn perf() -> &'static mut Perf {
    PERF.get(|| Perf {
        task_ring: Box::default(),
        task_len: 0,
        frame_ring: Box::default(),
        frame_len: 0,
        task_head: 0,
        frame_head: 0,
        truncated: false,
        recording: false,
        started_at: now(),
        stopped_at: None,
        worst: Box::default(),
        worst_len: 0,
        held: RecordingSection::default(),
        scratch: Box::default(),
        open: None,
        frame_task_ms: 0.0,
        read_turn: false,
    })
}

pub fn perf_recording() -> bool {
    perf().recording
}

/// The recording as NOW's rec section holds it, each flag 0 or 1.
pub fn perf_recording_state() -> RecSection {
    let l = perf();
    RecSection {
        recording: l.recording as u8 as f64,
        elapsed_ms: l.stopped_at.unwrap_or_else(now) - l.started_at,
        truncated: l.truncated as u8 as f64,
    }
}

/// Recording from now, its columns taken from the heap where it holds none
/// yet: false, nothing recorded, where the heap refuses them.
pub fn perf_recording_start() -> bool {
    let l = perf();
    if l.task_ring.is_empty() {
        let (Some(tasks), Some(frames), Some(worst), Some(scratch)) = (
            heap_column(TASK_LIMIT),
            heap_column(FRAME_LIMIT),
            heap_column(WORST_CAP),
            heap_column(PERCENTILES_LIMIT),
        ) else {
            return false;
        };
        (l.task_ring, l.frame_ring, l.worst, l.scratch) = (tasks, frames, worst, scratch);
    }
    (l.task_len, l.frame_len, l.worst_len) = (0, 0, 0);
    (l.task_head, l.frame_head) = (0, 0);
    l.open = None;
    l.held = RecordingSection::default();
    l.truncated = false;
    l.started_at = now();
    l.stopped_at = None;
    l.recording = true;
    true
}

pub fn perf_recording_stop() {
    let l = perf();
    if !l.recording {
        return;
    }
    l.recording = false;
    l.stopped_at = Some(now());
}

/// The columns given back to the heap, what they held with them: the
/// profiler's setting is off, so nothing shows them.
pub fn perf_release() {
    perf_recording_stop();
    let l = perf();
    (
        l.task_len,
        l.frame_len,
        l.worst_len,
        l.task_head,
        l.frame_head,
    ) = (0, 0, 0, 0, 0);
    (l.task_ring, l.frame_ring, l.worst, l.scratch) = (
        Box::default(),
        Box::default(),
        Box::default(),
        Box::default(),
    );
}

/// The read's scratch, empty while the columns are not held.
pub(crate) fn perf_scratch() -> &'static mut [f64] {
    &mut perf().scratch
}

/// `row` is made only while recording.
pub fn perf_record(row: impl FnOnce() -> TaskRow) {
    let l = perf();
    if !l.recording {
        return;
    }
    let row = row();
    let dropped = trim(
        &l.task_ring[..l.task_len],
        &mut l.task_head,
        row.end_ms - WINDOW_MS,
        |r| r.end_ms,
    );
    drop_front(&mut l.task_ring, &mut l.task_len, dropped);
    if !land_column(&mut l.task_ring, &mut l.task_len, &mut l.task_head, row) {
        l.truncated = true;
    }
}

/// A frame turn, `s` its record: closes the interval in progress with
/// `interval_ms`, the host's gap that just ended, and `settled`, whether the
/// host settled it, lands it, and opens `s`'s. `frame_task_ms` is how long
/// the frame task ran, the turn's end's to take out of the tasks' time.
pub fn perf_record_frame(s: FrameSample, interval_ms: f64, settled: bool, frame_task_ms: f64) {
    let l = perf();
    if !l.recording {
        return;
    }
    let at = now();
    l.frame_task_ms = frame_task_ms;
    if let Some(mut f) = l.open.replace(s) {
        (f.interval_ms, f.settled) = (interval_ms, f64::from(u8::from(settled)));
        land(l, f);
    }
    // Landing it is the profiler's too, out of the frame task's time.
    let ms = now() - at;
    l.frame_task_ms += ms;
    if let Some(f) = l.open.as_mut() {
        f.perf_ms += ms;
    }
}

/// `write` into the record of the frame the renderer numbered `frame`, as
/// a read of it lands a frame or two after it: the one in progress or one
/// landed, however late, and a worst frame's that holds it.
fn at_frame(l: &mut Perf, frame: f64, write: impl Fn(&mut FrameSample)) {
    if frame == 0.0 {
        return;
    }
    let of = |f: &mut FrameSample| {
        if f.gpu.frame == frame {
            write(f);
        }
    };
    if let Some(f) = l.open.as_mut() {
        of(f);
    }
    // Back from the newest to the frame it names.
    for f in l.frame_ring[l.frame_head..l.frame_len].iter_mut().rev() {
        of(f);
        if f.gpu.frame != 0.0 && f.gpu.frame <= frame {
            break;
        }
    }
    for w in &mut l.worst[..l.worst_len] {
        of(&mut w.sample);
    }
}

/// The GPU's read `r` of the frame the renderer numbered `r.frame`, written
/// into that frame's record.
pub fn perf_record_gpu(r: GpuRead) {
    let l = perf();
    if !l.recording {
        return;
    }
    at_frame(l, r.frame, |f| {
        (f.gpu.read, f.gpu.spans, f.gpu.busy_ms, f.gpu.px) = (1.0, r.spans, r.busy_ms, r.px);
    });
}

/// The ray counts `r` of the frame the renderer numbered `r.frame`, written
/// into that frame's record.
pub fn perf_record_rays(r: RayRead) {
    let l = perf();
    if !l.recording {
        return;
    }
    at_frame(l, r.frame, |f| f.rays = r.rays);
}

/// What a frame turn held, `held`, folded into the recording's extremes.
pub fn perf_fold_recording(held: &RecordingSection) {
    let l = perf();
    if l.recording {
        l.held.fold(held);
    }
}

pub fn perf_recording_held() -> RecordingSection {
    perf().held
}

/// A turn's end, frame turn or not: its time into the interval in progress,
/// rise to fall, its tasks' part, and what it asked as it fell, unless the
/// turn served the shell's read of the profiler.
pub fn perf_record_turn(turn_ms: f64, poll_ms: f64, wake: f64) {
    let l = perf();
    let frame_task_ms = core::mem::take(&mut l.frame_task_ms);
    let read_turn = core::mem::take(&mut l.read_turn);
    let Some(f) = l.open.as_mut().filter(|_| l.recording) else {
        return;
    };
    f.turns += 1.0;
    f.busy_ms += turn_ms;
    // The frame task runs inside the poll, so this is never negative.
    f.tasks_ms += poll_ms - frame_task_ms;
    if !read_turn {
        f.wake = wake;
    }
}

/// The shell's read of the profiler, `ms` long, served this turn: the
/// profiler's cost to the interval in progress (`perf_ms`).
pub fn perf_record_read(ms: f64) {
    let l = perf();
    l.read_turn = true;
    if let Some(f) = l.open.as_mut().filter(|_| l.recording) {
        f.perf_ms += ms;
    }
}

/// A still interval is long on purpose, so only a moving frame is among the
/// worst.
fn land(l: &mut Perf, s: FrameSample) {
    let at = l.worst[..l.worst_len].partition_point(|w| w.sample.interval_ms >= s.interval_ms);
    if s.settled == 0.0 && at < l.worst.len() {
        let kept = l.worst_len.min(l.worst.len() - 1);
        l.worst.copy_within(at..kept, at + 1);
        l.worst[at] = WorstFrame::of(s, l.frame_ring[..l.frame_len].last());
        l.worst_len = kept + 1;
    }

    let dropped = trim(
        &l.frame_ring[..l.frame_len],
        &mut l.frame_head,
        s.t - WINDOW_MS,
        |f| f.t,
    );
    drop_front(&mut l.frame_ring, &mut l.frame_len, dropped);
    if !land_column(&mut l.frame_ring, &mut l.frame_len, &mut l.frame_head, s) {
        l.truncated = true;
    }
}

fn first_at<T>(ring: &[T], t: f64, at: impl Fn(&T) -> f64) -> usize {
    ring.partition_point(|rec| at(rec) < t)
}

pub fn perf_frames() -> &'static [FrameSample] {
    let l = perf();
    &l.frame_ring[l.frame_head..l.frame_len]
}

/// In `end_ms` order, since a task is recorded as it settles.
pub fn perf_tasks() -> &'static [TaskRow] {
    let l = perf();
    &l.task_ring[l.task_head..l.task_len]
}

pub fn perf_worst() -> &'static [WorstFrame] {
    let l = perf();
    &l.worst[..l.worst_len]
}

pub fn perf_frames_since(t: f64) -> &'static [FrameSample] {
    let ring = perf_frames();
    &ring[first_at(ring, t, |f| f.t)..]
}

pub fn perf_tasks_since(t: f64) -> &'static [TaskRow] {
    let ring = perf_tasks();
    &ring[first_at(ring, t, |r| r.end_ms)..]
}
