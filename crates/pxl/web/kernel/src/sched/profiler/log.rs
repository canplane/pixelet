// pxl-kernel/sched/profiler/log.rs

//! The profiler's live stream: task rows and slow frames, kept in the
//! profiler's own lanes for the shell to take (`PROFILER_TAKE`). Not on the
//! ring: `Door::post` asserts on a full completion ring, so logging could
//! take the module down during the very fetch burst it was turned on to
//! watch. Only frames that missed a vsync while moving or overran are kept.

use crate::global::Global;
use crate::kernel::kernel;
use crate::layer::billboard::sheet::billboard_sheet;
use crate::sched::profiler::block::CountersBlock;
use crate::sched::profiler::record::{drop_front, fixed_lane, profiler_recording_start, profiler_recording_stop, profiler_set_sink, FrameSample, Lane, ProfilerSink, TaskRow};
use crate::sched::profiler::summary::{missed, overran};
use crate::sched::tick::{set_step_timing, tick_step_stats_reset};

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

/// Capped, oldest dropped, so a reader that stops cannot grow the heap being measured.
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

pub fn window_counters() -> CountersBlock {
    counters().minus(&log().baseline)
}

/// Called at WORLD_BOOT with the stored setting, before the first turn, so
/// a profiler left on has the boot in its window.
pub fn profiler_follow(on: bool) {
    if on {
        profiler_window_start();
    } else {
        profiler_set_logging(false);
        profiler_window_stop();
    }
}

/// Step timing starts here, not at boot: it is a clock read around every
/// step of every tick, and only the report reads it.
pub fn profiler_window_start() {
    profiler_recording_start();
    tick_step_stats_reset();
    set_step_timing(true);
    log().baseline = counters();
}

pub fn profiler_window_stop() {
    profiler_recording_stop();
    set_step_timing(false);
}

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

pub fn profiler_log_rows() -> (&'static [TaskRow], &'static [FrameSample]) {
    let l = log();
    (&l.tasks[..l.tasks_len], &l.frames[..l.frames_len])
}

pub fn profiler_log_clear() {
    let l = log();
    (l.tasks_len, l.frames_len) = (0, 0);
}
