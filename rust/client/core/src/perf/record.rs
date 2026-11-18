// pxl-core/perf/record.rs

//! Records where the core's time and work go. Each record is laid out word
//! for word as perf.json5 says, in tables. A read is one call that keeps
//! nothing: it copies what it answers, a page at most (`limit::PAGE_SZ`),
//! into its reply (link/export/perf.rs `read_timed`). The frames and the
//! tasks are read by sequence: each table counts the rows it has landed
//! (`frame_seq`, `task_seq`), a row's sequence is that count as the row
//! landed, and a read answers the rows from a sequence on with the sequence
//! of the first of them, so a reader that asks from where its last read ended
//! learns how many rows the table dropped before it came to them. A frame's
//! row is written again a frame or two after it lands (`at_frame`), so a
//! reader of the frames reads them from the oldest to the end in one pass.
//! The worst frames are one page, read together. The tables, the
//! percentiles' scratch and the failures' string table are blocks held on the
//! heap while the profiler's setting is on: they are taken whole when
//! recording starts and given back when the setting is turned off
//! (`perf_release`). The heap figures include them throughout, so they never
//! show up in a frame's heap delta.

use core::mem::size_of;

use pxl_global::Global;
use pxl_render::renderer::GpuRead;
use xpute_guest::clock::now;

use crate::perf::frame::{FrameSample, RaySample, WorstFrame};
use crate::perf::section::{RecordingSection, RecordingStateSection};
use crate::perf::task::TaskRecord;
use crate::spec::perf::limit;
use crate::spec::step::StepName;
use crate::sys::sched::tick::TickContext;

/// The tables keep the last minute (perf.json5 `LIMIT.WINDOW_MS`), as the UI's
/// long frames do. At 60 Hz that is 3,600 frames. A faster display or a busy
/// minute fills a table early, and the table then drops its oldest records
/// and sets `truncated`.
const WINDOW_MS: f64 = limit::WINDOW_MS as f64;
const TASK_CAP: usize = 2_048;
const FRAME_CAP: usize = 4_096;
/// The worst frames the profiler keeps, which one read answers in one page.
const WORST_CAP: usize = 9;
const _: () = assert!(WORST_CAP * size_of::<WorstFrame>() <= limit::PAGE_SZ as usize);
/// The bytes of the failures' file names, which are few.
pub(crate) const STRTAB_CAP: usize = 8_192;
pub(crate) const PERCENTILE_CAP: usize = if FRAME_CAP > TASK_CAP {
    FRAME_CAP
} else {
    TASK_CAP
};

/// Advances `head` past records older than `cutoff`. Returns how many records
/// to drop from the front now, which stays 0 until the expired part is at
/// least half the table, so records are shifted rarely.
fn trim<T>(table: &[T], head: &mut usize, cutoff: f64, t: impl Fn(&T) -> f64) -> usize {
    while *head < table.len() && t(&table[*head]) < cutoff {
        *head += 1;
    }
    if *head > 0 && *head * 2 >= table.len() {
        core::mem::take(head)
    } else {
        0
    }
}

/// Appends `rec` to the table, as the row of sequence `*seq`, which it
/// advances. A full table first drops its oldest quarter, so it shifts once a
/// quarter of the table rather than once a record, and the call then returns
/// false.
fn land_row<T: Copy>(
    table: &mut [T],
    len: &mut usize,
    head: &mut usize,
    seq: &mut u64,
    rec: T,
) -> bool {
    let full = *len == table.len() && *head == 0;
    if *len == table.len() {
        let n = if full {
            table.len() / 4
        } else {
            core::mem::take(head)
        };
        drop_front(table, len, n);
    }
    table[*len] = rec;
    *len += 1;
    *seq += 1;
    !full
}

pub(super) fn drop_front<T: Copy>(table: &mut [T], len: &mut usize, n: usize) {
    table.copy_within(n..*len, 0);
    *len -= n;
}

/// The byte size of each block, in order: tasks, frames, the worst frames,
/// the read's scratch (read.rs) and the string table.
const BLOCKS: [u32; 5] = [
    (TASK_CAP * size_of::<TaskRecord>()) as u32,
    (FRAME_CAP * size_of::<FrameSample>()) as u32,
    (WORST_CAP * size_of::<WorstFrame>()) as u32,
    (PERCENTILE_CAP * size_of::<f64>()) as u32,
    STRTAB_CAP as u32,
];

/// The heap the profiler holds while its setting is on. The buddy allocator
/// gives each table a power-of-two block, so each table counts at its size
/// rounded up (link/layout.rs `HEAP_FLOWS_SZ`).
pub(crate) const PERF_SZ: u64 = {
    let mut sum = 0;
    let mut k = 0;
    while k < BLOCKS.len() {
        sum += (BLOCKS[k] as u64).next_power_of_two();
        k += 1;
    }
    sum
};

/// A table of `n` default records, each a valid record. Returns None when
/// the heap refuses it.
fn heap_block<T: Copy + Default>(n: usize) -> Option<Box<[T]>> {
    let mut table = Vec::new();
    table.try_reserve_exact(n).ok()?;
    table.resize(n, T::default());
    Some(table.into_boxed_slice())
}

struct Perf {
    task_table: Box<[TaskRecord]>,
    task_len: usize,
    frame_table: Box<[FrameSample]>,
    frame_len: usize,
    task_head: usize,
    frame_head: usize,
    /// The rows each table has landed since the core started, the sequence
    /// its next row takes. Neither is set back, by a recording's start or by
    /// the blocks' release, so a sequence names one row for as long as the
    /// core runs.
    task_seq: u64,
    frame_seq: u64,
    truncated: bool,
    /// Off until the UI asks, so a client without the profiler on records
    /// nothing. Stopping freezes the data so an export is stable.
    recording: bool,
    started_at: f64,
    stopped_at: Option<f64>,
    worst: Box<[WorstFrame]>,
    worst_len: usize,
    /// The largest and smallest values the frames held since the recording
    /// started (`perf_fold_recording`).
    extreme: RecordingSection,
    /// Where the read sorts the values it takes percentiles of (read.rs).
    scratch: Box<[f64]>,
    /// The file names of the failures' sites, each at the offset a task's row
    /// names it at (`at_file`), and NUL-ended, as ELF's string table holds a
    /// file's names: offset 0 is the empty name, which no site has. Its
    /// first `strtab_len` bytes are taken.
    strtab: Box<[u8]>,
    strtab_len: usize,
    /// The interval in progress: the last frame's record, which the profiler's
    /// reads add their time to until the next frame closes it.
    open: Option<FrameSample>,
}

static PERF: Global<Perf> = Global::new();

fn perf() -> &'static mut Perf {
    PERF.get(|| Perf {
        task_table: Box::default(),
        task_len: 0,
        frame_table: Box::default(),
        frame_len: 0,
        task_head: 0,
        frame_head: 0,
        task_seq: 0,
        frame_seq: 0,
        truncated: false,
        recording: false,
        started_at: now(),
        stopped_at: None,
        worst: Box::default(),
        worst_len: 0,
        extreme: RecordingSection::default(),
        scratch: Box::default(),
        strtab: Box::default(),
        strtab_len: 0,
        open: None,
    })
}

pub(crate) fn perf_recording() -> bool {
    perf().recording
}

/// The recording as NOW's recording_state section holds it, each flag 0 or 1.
pub(crate) fn perf_recording_state() -> RecordingStateSection {
    let l = perf();
    RecordingStateSection {
        recording: l.recording as u8 as f64,
        elapsed_ms: l.stopped_at.unwrap_or_else(now) - l.started_at,
        truncated: l.truncated as u8 as f64,
        frame_cnt: perf_frames().len() as f64,
        task_cnt: perf_tasks().len() as f64,
    }
}

/// Starts a recording from now, taking the blocks from the heap if they are
/// not held yet. Returns false, and records nothing, when the heap refuses
/// them.
pub(crate) fn perf_recording_start() -> bool {
    let l = perf();
    if l.task_table.is_empty() {
        let (Some(tasks), Some(frames), Some(worst), Some(scratch), Some(strtab)) = (
            heap_block(TASK_CAP),
            heap_block(FRAME_CAP),
            heap_block(WORST_CAP),
            heap_block(PERCENTILE_CAP),
            heap_block(STRTAB_CAP),
        ) else {
            return false;
        };
        (l.task_table, l.frame_table, l.worst, l.scratch, l.strtab) =
            (tasks, frames, worst, scratch, strtab);
    }
    (l.task_len, l.frame_len, l.worst_len) = (0, 0, 0);
    // Offset 0 holds the empty name.
    l.strtab_len = 1;
    (l.task_head, l.frame_head) = (0, 0);
    l.open = None;
    l.extreme = RecordingSection::default();
    l.truncated = false;
    l.started_at = now();
    l.stopped_at = None;
    l.recording = true;
    true
}

pub(crate) fn perf_recording_stop() {
    let l = perf();
    if !l.recording {
        return;
    }
    l.recording = false;
    l.stopped_at = Some(now());
}

/// Gives the blocks, and what they hold, back to the heap. It runs when the
/// profiler's setting goes off, after which nothing shows them.
pub(crate) fn perf_release() {
    perf_recording_stop();
    let l = perf();
    (
        l.task_len,
        l.frame_len,
        l.worst_len,
        l.task_head,
        l.frame_head,
        l.strtab_len,
    ) = (0, 0, 0, 0, 0, 0);
    (l.task_table, l.frame_table, l.worst, l.scratch, l.strtab) = (
        Box::default(),
        Box::default(),
        Box::default(),
        Box::default(),
        Box::default(),
    );
}

/// The read's scratch, empty while the blocks are not held.
pub(crate) fn perf_scratch() -> &'static mut [f64] {
    &mut perf().scratch
}

/// Records a settled task. `row` is built only while recording, and it names
/// a failure's file through the string table it is handed. A name the table
/// has no room for is lost, as a record a full table drops is (`truncated`).
pub(crate) fn perf_record(
    row: impl FnOnce(&mut dyn FnMut(&'static str) -> Option<u32>) -> TaskRecord,
) {
    let l = perf();
    if !l.recording {
        return;
    }
    let mut lost = false;
    let row = row(&mut |file| {
        let off = name_at(&mut l.strtab, &mut l.strtab_len, file);
        lost |= off.is_none();
        off
    });
    l.truncated |= lost;
    let dropped = trim(
        &l.task_table[..l.task_len],
        &mut l.task_head,
        row.end_ms - WINDOW_MS,
        |r| r.end_ms,
    );
    drop_front(&mut l.task_table, &mut l.task_len, dropped);
    let landed = land_row(
        &mut l.task_table,
        &mut l.task_len,
        &mut l.task_head,
        &mut l.task_seq,
        row,
    );
    l.truncated |= !landed;
}

/// Records a frame whose record is `s`. It closes the interval in progress
/// with `interval_ms`, the gap that just ended, then lands that interval and
/// opens `s`'s.
pub(crate) fn perf_record_frame(s: FrameSample, interval_ms: f64) {
    let l = perf();
    if !l.recording {
        return;
    }
    let at = now();
    if let Some(mut f) = l.open.replace(s) {
        f.interval_ms = interval_ms;
        land(l, f);
    }
    // Landing the record is profiler time too, so it goes into `perf_ms`.
    let ms = now() - at;
    if let Some(f) = l.open.as_mut() {
        f.perf_ms += ms;
    }
}

/// Applies `write` to the record of the frame the renderer numbered `frame`.
/// A read lands a frame or two after its frame, so the record may be the one
/// in progress or one already landed, however late. A worst frame may hold a
/// copy of the record too.
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
    // Walk back from the newest record to the frame named.
    for f in l.frame_table[l.frame_head..l.frame_len].iter_mut().rev() {
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
pub(crate) fn perf_record_gpu(r: GpuRead) {
    let l = perf();
    if !l.recording {
        return;
    }
    at_frame(l, r.frame, |f| {
        (f.gpu.timed, f.gpu.spans, f.gpu.busy_ms, f.gpu.px) = (1.0, r.spans, r.busy_ms, r.px);
    });
}

/// A frame's ray counts as they land, with the renderer's number for the
/// frame that counted them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RayRead {
    pub frame: f64,
    pub sample: RaySample,
}

/// The ray counts `r` of the frame the renderer numbered `r.frame`, written
/// into that frame's record.
pub(crate) fn perf_record_rays(r: RayRead) {
    let l = perf();
    if !l.recording {
        return;
    }
    at_frame(l, r.frame, |f| f.ray_sample = r.sample);
}

/// Folds what a frame held, `section`, into the recording's extremes.
pub(crate) fn perf_fold_recording(section: &RecordingSection) {
    let l = perf();
    if l.recording {
        l.extreme.fold(section);
    }
}

pub(crate) fn perf_recording_extremes() -> RecordingSection {
    perf().extreme
}

/// Records that an entry answered the UI's read of the profiler, which took
/// `ms`. That time is the profiler's cost to the interval in progress
/// (`perf_ms`).
pub(crate) fn perf_record_read(ms: f64) {
    let l = perf();
    if let Some(f) = l.open.as_mut().filter(|_| l.recording) {
        f.perf_ms += ms;
    }
}

/// Records the ground's fill, in the entry after the frame whose interval is
/// in progress: it took `ms` and filled `cnt` bricks.
pub(crate) fn perf_record_fill(ms: f64, cnt: u32) {
    let l = perf();
    if let Some(f) = l.open.as_mut().filter(|_| l.recording) {
        f.fill_ms += ms;
        f.fill_cnt += f64::from(cnt);
    }
}

/// Lands a closed interval's record. A settled interval is long on purpose,
/// so only a moving frame can be among the worst.
fn land(l: &mut Perf, s: FrameSample) {
    let at = l.worst[..l.worst_len].partition_point(|w| w.sample.interval_ms >= s.interval_ms);
    if s.settled == 0.0 && at < l.worst.len() {
        let end = l.worst_len.min(l.worst.len() - 1);
        l.worst.copy_within(at..end, at + 1);
        l.worst[at] = WorstFrame::of(s, l.frame_table[..l.frame_len].last());
        l.worst_len = end + 1;
    }

    let dropped = trim(
        &l.frame_table[..l.frame_len],
        &mut l.frame_head,
        s.t - WINDOW_MS,
        |f| f.t,
    );
    drop_front(&mut l.frame_table, &mut l.frame_len, dropped);
    let landed = land_row(
        &mut l.frame_table,
        &mut l.frame_len,
        &mut l.frame_head,
        &mut l.frame_seq,
        s,
    );
    l.truncated |= !landed;
}

/// The offset of `file` in the string table `t`, whose first `*len` bytes are
/// taken: where the table holds it already, or else after them, NUL-ended.
/// None where the table has no room for it.
fn name_at(t: &mut [u8], len: &mut usize, file: &str) -> Option<u32> {
    let mut at = 1;
    for name in t.get(1..*len)?.split(|&b| b == 0) {
        if name == file.as_bytes() {
            return Some(at as u32);
        }
        at += name.len() + 1;
    }
    let end = *len + file.len() + 1;
    let room = t.get_mut(*len..end)?;
    room[..file.len()].copy_from_slice(file.as_bytes());
    room[file.len()] = 0;
    let off = *len as u32;
    *len = end;
    Some(off)
}

/// The string table's names so far, empty while the blocks are not held.
pub(crate) fn perf_strtab() -> &'static [u8] {
    let l = perf();
    &l.strtab[..l.strtab_len]
}

fn first_at<T>(table: &[T], t: f64, at: impl Fn(&T) -> f64) -> usize {
    table.partition_point(|rec| at(rec) < t)
}

pub(crate) fn perf_frames() -> &'static [FrameSample] {
    let l = perf();
    &l.frame_table[l.frame_head..l.frame_len]
}

/// The tasks lie in `end_ms` order, since a task is recorded as it settles.
pub(crate) fn perf_tasks() -> &'static [TaskRecord] {
    let l = perf();
    &l.task_table[l.task_head..l.task_len]
}

/// The sequence past the newest frame's row, and past the newest task's.
pub(crate) fn perf_frame_seq() -> u64 {
    perf().frame_seq
}

pub(crate) fn perf_task_seq() -> u64 {
    perf().task_seq
}

/// The worst frames, longest first.
pub(crate) fn perf_worst() -> &'static [WorstFrame] {
    let l = perf();
    &l.worst[..l.worst_len]
}

pub(crate) fn perf_frames_since(t: f64) -> &'static [FrameSample] {
    let table = perf_frames();
    &table[first_at(table, t, |f| f.t)..]
}

pub(crate) fn perf_tasks_since(t: f64) -> &'static [TaskRecord] {
    let table = perf_tasks();
    &table[first_at(table, t, |r| r.end_ms)..]
}

#[cfg(test)]
#[path = "record.test.rs"]
mod test;

/// A step's time, summed while the profiler records.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StepStat {
    pub ms: f64,
    pub cnt: u32,
    pub max_ms: f64,
}

const STAT_NONE: StepStat = StepStat {
    ms: 0.0,
    cnt: 0,
    max_ms: 0.0,
};

/// Each step's time by its name, since the profiler last began to record.
static STEP_STATS: Global<[StepStat; StepName::ALL.len()]> = Global::new();
/// Whether the profiler records, and the steps are timed.
static STEP_TIMING: Global<bool> = Global::new();

fn step_stats_mut() -> &'static mut [StepStat; StepName::ALL.len()] {
    STEP_STATS.get(|| [STAT_NONE; StepName::ALL.len()])
}

/// Runs the step `name`, timed while the profiler records.
pub(crate) fn timed(ctx: &mut TickContext, name: StepName, step: fn(&mut TickContext)) {
    if !*STEP_TIMING.get(|| false) {
        return step(ctx);
    }
    let at = now();
    step(ctx);
    let ms = now() - at;
    let s = &mut step_stats_mut()[name as usize];
    (s.ms, s.cnt, s.max_ms) = (s.ms + ms, s.cnt + 1, s.max_ms.max(ms));
}

/// Starts summing each step's time from zero, or stops summing it; what was
/// summed stays to be read.
pub(crate) fn set_step_timing(on: bool) {
    if on {
        *step_stats_mut() = [STAT_NONE; StepName::ALL.len()];
    }
    STEP_TIMING.set(on);
}

/// Each step that ran while the profiler recorded, by its name.
pub(crate) fn step_stats() -> impl Iterator<Item = (StepName, StepStat)> {
    let stats = *step_stats_mut();
    StepName::ALL
        .into_iter()
        .map(move |name| (name, stats[name as usize]))
        .filter(|(_, s)| s.cnt > 0)
}
