// pxl-kernel/sched/ledger/log.rs

//! The ledger's live stream — what the gauge panel cannot do: records as they
//! land, so a stutter can be watched happening and correlated against whatever
//! else is logging, rather than read off a panel that only shows the settled
//! aggregate.
//!
//! What is streamed is what was measured. The lines a person reads are the
//! shell's (apps/web/src/feature/dev/ledger/report.ts), the same
//! formatters the report already uses, so a streamed task and a reported one
//! read alike without this side knowing a word of it.
//!
//! It does not go out on the ring. The ring is the control path — ordered,
//! back-pressured, and full is fatal: `Outbox::post` asserts when the
//! completion queue and its spill are both full, so a debug line could take
//! the module down, and the moment logging is turned on is exactly the moment
//! instrumentation would be competing for ring slots with the fetch burst it
//! was turned on to watch. This is what every operating system separates —
//! ftrace and perf mmap their own per-CPU buffers, printk keeps `log_buf` and
//! is read through /dev/kmsg, eBPF has its own ring — so the stream is two
//! lanes of the ledger's own records the consumer reads in place at its own
//! pace, through pxl-wasm's `ledger_log`, and clears when it has read them.
//!
//! A dev instrument that never feeds back into scheduling. The stream
//! thresholds frames deliberately: an unfiltered per-frame record would bury
//! everything else within seconds, so only frames that missed a vsync while
//! moving, or overran the budget, are kept — summary.rs's `missed` and
//! `overran`, the same tests the report counts.

use crate::global::Global;
use crate::kernel::kernel;
use crate::layer::poi::badge::badge_atlas;
use crate::sched::ledger::record::{drop_front, fixed_lane, ledger_recording_start, ledger_recording_stop, ledger_set_sink, FrameSample, LedgerSink, TaskRow};
use crate::sched::ledger::summary::{missed, overran};
use crate::sched::tick::{set_step_timing, tick_step_stats_reset};

/// The side counters printed beside the rings, cumulative from load.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counters {
    pub faces_kept: f64,
    pub faces_possible: f64,
    pub runs_culled: f64,
    pub runs_total: f64,
    pub coarse_boxes: f64,
    pub coarse_low_fill: f64,
    pub bands_reused: f64,
    pub bands_allocated: f64,
    pub built_direct: f64,
    pub built_scans: f64,
    pub built_bakes: f64,
    pub built_starved: f64,
    pub decor_dropped: f64,
    pub badge_metrics: f64,
    pub badge_builds: f64,
    pub badge_ms: f64,
    pub place_runs: f64,
    pub atlas_repacks: f64,
    pub atlas_uploads: f64,
    pub index_rebuilds: f64,
}

impl Counters {
    fn minus(self, b: Counters) -> Counters {
        Counters {
            faces_kept: self.faces_kept - b.faces_kept,
            faces_possible: self.faces_possible - b.faces_possible,
            runs_culled: self.runs_culled - b.runs_culled,
            runs_total: self.runs_total - b.runs_total,
            coarse_boxes: self.coarse_boxes - b.coarse_boxes,
            coarse_low_fill: self.coarse_low_fill - b.coarse_low_fill,
            bands_reused: self.bands_reused - b.bands_reused,
            bands_allocated: self.bands_allocated - b.bands_allocated,
            built_direct: self.built_direct - b.built_direct,
            built_scans: self.built_scans - b.built_scans,
            built_bakes: self.built_bakes - b.built_bakes,
            built_starved: self.built_starved - b.built_starved,
            decor_dropped: self.decor_dropped - b.decor_dropped,
            badge_metrics: self.badge_metrics - b.badge_metrics,
            badge_builds: self.badge_builds - b.badge_builds,
            badge_ms: self.badge_ms - b.badge_ms,
            place_runs: self.place_runs - b.place_runs,
            atlas_repacks: self.atlas_repacks - b.atlas_repacks,
            atlas_uploads: self.atlas_uploads - b.atlas_uploads,
            index_rebuilds: self.index_rebuilds - b.index_rebuilds,
        }
    }
}

fn counters() -> Counters {
    let k = kernel();
    let st = k.sched.builder.stat;
    let c = k.sched.builder.bake_context().counters;
    let bands = k.volumes.as_ref().map_or([0; 2], |v| [v.stat.allocated, v.stat.reused]);
    let atlas = badge_atlas().stats();
    Counters {
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
        decor_dropped: k.sched.decor_dropped() as f64,
        badge_metrics: 0.0,
        badge_builds: 0.0,
        badge_ms: 0.0,
        place_runs: 0.0,
        atlas_repacks: atlas.repacks as f64,
        atlas_uploads: atlas.uploads as f64,
        index_rebuilds: 0.0,
    }
}

/// What the stream holds for the shell to come and get. A consumer that stops
/// reading must not grow the heap the thing being measured allocates from, so
/// each lane is capped and the oldest goes — printk's ring loses the oldest
/// line too, rather than the producer waiting on the reader.
const PENDING: usize = 512;

struct Log {
    baseline: Counters,
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
pub fn window_counters() -> Counters {
    counters().minus(log().baseline)
}

/// Clears the ledger and opens a fresh window, side counters included.
///
/// Timing the steps one by one starts here rather than at boot, for the same
/// reason recording does: it is a clock read on either side of every step of
/// every phase of every tick, and only the report reads the result.
pub fn ledger_window_start() {
    ledger_recording_start();
    tick_step_stats_reset();
    set_step_timing(true);
    log().baseline = counters();
}

/// Freezes the window, so an export taken afterwards sees exactly this data,
/// and stops paying for the timing that fills it.
pub fn ledger_window_stop() {
    ledger_recording_stop();
    set_step_timing(false);
}

/// Every registered step by mean ms per run, heaviest first.
pub fn ledger_logging() -> bool {
    log().logging
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
/// ledger's single sink.
pub fn ledger_set_logging(on: bool) {
    let l = log();
    if on == l.logging {
        return;
    }
    l.logging = on;
    if on && l.tasks.is_empty() {
        l.tasks = fixed_lane(PENDING);
        l.frames = fixed_lane(PENDING);
    }
    ledger_set_sink(on.then_some(LedgerSink {
        task: Some(log_task),
        frame: Some(log_frame),
    }));
}

/// The rows the stream kept since the last clear, oldest first: tasks, then
/// frames.
pub fn ledger_log_rows() -> (&'static [TaskRow], &'static [FrameSample]) {
    let l = log();
    (&l.tasks[..l.tasks_len], &l.frames[..l.frames_len])
}

/// Forgets what the stream kept, once the shell has read it.
pub fn ledger_log_clear() {
    let l = log();
    (l.tasks_len, l.frames_len) = (0, 0);
}
