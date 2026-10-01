// pxl-kernel/sched/profiler/log.rs

//! The profiler's live stream — what the gauge panel cannot do: records as they
//! land, so a stutter can be watched happening and correlated against whatever
//! else is logging, rather than read off a panel that only shows the settled
//! aggregate.
//!
//! What is streamed is what was measured. The lines a person reads are the
//! shell's (apps/web/src/feature/debug/profiler/report.ts), the same
//! formatters the report already uses, so a streamed task and a reported one
//! read alike without this side knowing a word of it.
//!
//! It does not go out on the ring. The ring is the control path — ordered,
//! back-pressured, and full is fatal: `Door::post` asserts when the
//! completion ring is full, so a debug line could take
//! the module down, and the moment logging is turned on is exactly the moment
//! instrumentation would be competing for ring slots with the fetch burst it
//! was turned on to watch. This is what every operating system separates —
//! ftrace and perf mmap their own per-CPU buffers, printk keeps `log_buf` and
//! is read through /dev/kmsg, eBPF has its own ring — so the stream is two
//! lanes of the profiler's own records the consumer reads in place at its own
//! pace, through `PROFILER_TAKE`, and clears when it has read them.
//!
//! A dev instrument that never feeds back into scheduling. The stream
//! thresholds frames deliberately: an unfiltered per-frame record would bury
//! everything else within seconds, so only frames that missed a vsync while
//! moving, or overran the budget, are kept — summary.rs's `missed` and
//! `overran`, the same tests the report counts.

use crate::global::Global;
use crate::kernel::kernel;
use crate::layer::billboard::sheet::billboard_sheet;
use crate::sched::profiler::block::CountersBlock;
use crate::sched::profiler::record::{drop_front, fixed_lane, profiler_recording_start, profiler_recording_stop, profiler_set_sink, FrameSample, Lane, ProfilerSink, TaskRow};
use crate::sched::profiler::summary::{missed, overran};
use crate::sched::tick::{set_step_timing, tick_step_stats_reset};

/// The side counters printed beside the rings, cumulative from load.
fn counters() -> CountersBlock {
    let k = kernel();
    let st = k.sched.builder.stat;
    let c = k.sched.builder.bake_context().counters;
    let bands = k.volumes.as_ref().map_or([0; 2], |v| [v.stat.allocated, v.stat.reused]);
    let atlas = billboard_sheet().stats();
    CountersBlock {
        faces_kept: c.faces_kept as f64,
        faces_possible: c.faces_possible as f64,
        runs_culled: c.culled as f64,
        runs_total: c.total_runs as f64,
        coarse_boxes: c.coarse_total as f64,
        coarse_low_fill: c.coarse_low_fill as f64,
        bands_reused: bands[1] as f64,
        bands_allocated: bands[0] as f64,
        built_direct: st.direct as f64,
        built_scans: st.scans as f64,
        built_bakes: st.bakes as f64,
        built_starved: st.starved as f64,
        atlas_repacks: atlas.repacks as f64,
        atlas_uploads: atlas.uploads as f64,
    }
}

/// What the stream holds for the shell to come and get. A consumer that stops
/// reading must not grow the heap the thing being measured allocates from, so
/// each lane is capped and the oldest goes — printk's ring loses the oldest
/// line too, rather than the producer waiting on the reader.
pub(super) const PENDING: usize = 512;

struct Log {
    baseline: CountersBlock,
    logging: bool,
    tasks: &'static mut [TaskRow],
    tasks_len: usize,
    frames: &'static mut [FrameSample],
    frames_len: usize,
}

static LOG: Global<Log> = Global::new();

fn log() -> &'static mut Log {
    LOG.get(|| Log {
        baseline: counters(),
        logging: false,
        tasks: &mut [],
        tasks_len: 0,
        frames: &mut [],
        frames_len: 0,
    })
}

/// The side counters since the window began.
pub fn window_counters() -> CountersBlock {
    counters().minus(&log().baseline)
}

/// The profiler follows its setting, `profiler`: it records while the
/// setting is on and not at all while it is off. The store calls this with
/// what storage kept at WORLD_BOOT, before the first turn, so a profiler
/// left on has the boot in its window — the first turns and the first fetches,
/// which a start sent once the shell had drawn its panel came too late for.
pub fn profiler_follow(on: bool) {
    if on {
        profiler_window_start();
    } else {
        profiler_set_logging(false);
        profiler_window_stop();
    }
}

/// Clears the profiler and opens a fresh window, side counters included.
///
/// Timing the steps one by one starts here rather than at boot, for the same
/// reason recording does: it is a clock read on either side of every step of
/// every phase of every tick, and only the report reads the result.
pub fn profiler_window_start() {
    profiler_recording_start();
    tick_step_stats_reset();
    set_step_timing(true);
    log().baseline = counters();
}

/// Freezes the window, so an export taken afterward sees exactly this data,
/// and stops paying for the timing that fills it.
pub fn profiler_window_stop() {
    profiler_recording_stop();
    set_step_timing(false);
}

/// `one` onto a lane, the oldest dropped when it is full.
fn keep<T: Copy>(lane: &mut [T], len: &mut usize, one: T) {
    if lane.is_empty() {
        return;
    }
    if *len == lane.len() {
        drop_front(lane, len, 1);
    }
    lane[*len] = one;
    *len += 1;
}

fn log_task(rec: &TaskRow) {
    let l = log();
    keep(l.tasks, &mut l.tasks_len, *rec);
}

fn log_frame(s: &FrameSample) {
    if missed(s) || overran(s) {
        let l = log();
        keep(l.frames, &mut l.frames_len, *s);
    }
}

/// Turns the live console stream on/off by installing (or clearing) the
/// profiler's single sink.
pub fn profiler_set_logging(on: bool) {
    let l = log();
    if on == l.logging {
        return;
    }
    l.logging = on;
    if on && l.tasks.is_empty() {
        l.tasks = fixed_lane(Lane::LogTasks, PENDING);
        l.frames = fixed_lane(Lane::LogFrames, PENDING);
    }
    profiler_set_sink(on.then_some(ProfilerSink {
        task: Some(log_task),
        frame: Some(log_frame),
    }));
}

/// The rows the stream kept since the last clear, oldest first: tasks, then
/// frames.
pub fn profiler_log_rows() -> (&'static [TaskRow], &'static [FrameSample]) {
    let l = log();
    (&l.tasks[..l.tasks_len], &l.frames[..l.frames_len])
}

/// Forgets what the stream kept, once the shell has read it.
pub fn profiler_log_clear() {
    let l = log();
    (l.tasks_len, l.frames_len) = (0, 0);
}
