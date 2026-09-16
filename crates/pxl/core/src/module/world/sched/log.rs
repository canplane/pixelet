// pxl_core/module/world/sched/log.rs

//! The ledger's live stream — what the gauge panel cannot do: records as they
//! land, so a stutter can be watched happening and correlated against whatever
//! else is logging, rather than read off a panel that only shows the settled
//! aggregate.
//!
//! What is streamed is what was measured. The lines a person reads are the
//! shell's (apps/web/src/module/world/sched/ledger_report.ts), the same
//! formatters the report already uses, so a streamed task and a reported one
//! read alike without this side knowing a word of it.
//!
//! It does not go out on the ring. The ring is the control path — ordered,
//! back-pressured, and full is fatal: `Outbox::post` asserts when the
//! completion queue and its spill are both full, so a debug line could take
//! the module down, and the moment logging is turned on is exactly the moment
//! instrumentation would be competing for arena bytes with the fetch burst it
//! was turned on to watch. This is what every operating system separates —
//! ftrace and perf mmap their own per-CPU buffers, printk keeps `log_buf` and
//! is read through /dev/kmsg, eBPF has its own ring — so the stream is a
//! buffer the consumer reads at its own pace, through pxl-wasm's `ledger_log`.
//!
//! A dev instrument that never feeds back into scheduling. The stream
//! thresholds frames deliberately: an unfiltered per-frame record would bury
//! everything else within seconds, so only frames that missed a vsync while
//! moving, or overran the budget, are kept — summary.rs's `missed` and
//! `overran`, the same tests the report counts.

use super::ledger::{ledger_recording_start, ledger_set_sink, FrameSample, LedgerSink, LongFrameRecord, TaskRecord};
use super::summary::{missed, overran};
use super::tick::tick_step_stats_reset;
use crate::global::Global;
use crate::kernel::kernel;
use crate::module::world::poi::poi_badge::badge_atlas;


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
/// each is capped and the oldest goes — printk's ring loses the oldest line
/// too, rather than the producer waiting on the reader.
const PENDING: usize = 512;

struct Log {
    baseline: Counters,
    logging: bool,
    tasks: Vec<TaskRecord>,
    frames: Vec<FrameSample>,
    longs: Vec<LongFrameRecord>,
}

static LOG: Global<Log> = Global::new();

fn log() -> &'static mut Log {
    LOG.get(|| Log {
        baseline: counters(),
        logging: false,
        tasks: Vec::new(),
        frames: Vec::new(),
        longs: Vec::new(),
    })
}

/// The side counters since the window began.
pub fn window_counters() -> Counters {
    counters().minus(log().baseline)
}

/// Clears the ledger and opens a fresh window, side counters included.
pub fn ledger_window_start() {
    ledger_recording_start();
    tick_step_stats_reset();
    log().baseline = counters();
}

/// Every registered step by mean ms per run, heaviest first.
pub fn ledger_logging() -> bool {
    log().logging
}

fn keep<T>(held: &mut Vec<T>, one: T) {
    if held.len() == PENDING {
        held.remove(0);
    }
    held.push(one);
}

fn log_task(rec: &TaskRecord) {
    keep(&mut log().tasks, rec.clone());
}

fn log_frame(s: &FrameSample) {
    if missed(s) || overran(s) {
        keep(&mut log().frames, *s);
    }
}

fn log_long_frame(l: &LongFrameRecord) {
    keep(&mut log().longs, *l);
}

/// Turns the live console stream on/off by installing (or clearing) the
/// ledger's single sink.
pub fn ledger_set_logging(on: bool) {
    let l = log();
    if on == l.logging {
        return;
    }
    l.logging = on;
    ledger_set_sink(on.then_some(LedgerSink {
        task: Some(log_task),
        frame: Some(log_frame),
        long_frame: Some(log_long_frame),
    }));
}

/// The records the stream kept since the last take, cleared by taking.
pub fn take_log_records() -> (Vec<TaskRecord>, Vec<FrameSample>, Vec<LongFrameRecord>) {
    let l = log();
    (core::mem::take(&mut l.tasks), core::mem::take(&mut l.frames), core::mem::take(&mut l.longs))
}

/// Whether anything is waiting, so a turn that has nothing to say costs the
/// shell no call.
pub fn log_pending() -> bool {
    let l = log();
    !l.tasks.is_empty() || !l.frames.is_empty() || !l.longs.is_empty()
}
