// pxl_core/module/world/sched/log.rs

//! The ledger's live console stream — what the gauge panel cannot do:
//!
//! - a **live console stream** of records as they land, so a stutter can be
//!   watched happening and correlated against whatever else is logging, rather
//!   than read off a panel that only shows the settled aggregate.
//!
//! The report the rings are read as is report.rs, which carries the numbers
//! and leaves the words to the shell; this side still writes its own lines,
//! because a console stream is one line at a time and has nowhere to put a
//! document.
//!
//! A dev instrument that never feeds back into scheduling. The live stream
//! thresholds frames deliberately: an unfiltered per-frame line would
//! bury everything else in the console within seconds, so only frames that
//! missed a vsync while moving, or overran the budget, are worth a line —
//! summary.rs's `missed` and `overran`, the same tests the report counts. The
//! console itself is the shell's: lines are handed over by `take_log_lines`.

use pxl_shared::spec::fetch::FetchFail;

use super::demand::Phases;
use super::ledger::{ledger_recording_start, ledger_set_sink, FrameSample, LedgerSink, LongFrameRecord, LongFrameSource, TaskRecord};
use super::summary::{missed, overran};
use super::tick::tick_step_stats_reset;
use crate::global::Global;
use crate::kernel::kernel;
use crate::module::world::poi::poi_badge::badge_atlas;


fn n1(n: f64) -> String {
    format!("{n:.1}")
}

fn phases_text(phases: Option<&Phases>) -> String {
    let Some(phases) = phases.filter(|p| !p.is_empty()) else { return String::new() };
    let parts: Vec<String> = phases.iter().map(|(k, v)| format!("{} {}", k.trim_end_matches("_ms"), if k == "bytes" { v.to_string() } else { n1(v) })).collect();
    format!("  [{}]", parts.join(" "))
}

fn task_line(rec: &TaskRecord) -> String {
    let total = rec.end_ms - rec.enqueue_ms;
    let timing = match rec.start_ms {
        Some(start) => format!(" q{} r{}", n1(start - rec.enqueue_ms), n1(rec.end_ms - start)),
        None => String::new(),
    };
    format!(
        "[{:>9}] {:<34} {:<7} {:>8}ms{timing} p{}{}{}",
        n1(rec.enqueue_ms),
        rec.key,
        rec.outcome.as_str(),
        n1(total),
        rec.priority,
        phases_text(rec.phases.as_ref()),
        if rec.fail.fail == FetchFail::None { String::new() } else { format!("  [{}]", rec.fail) }
    )
}

fn signed(n: f64) -> String {
    if n >= 0.0 {
        format!("+{n}")
    } else {
        format!("{n}")
    }
}

/// `prev` is the frame immediately before this one in the ring, when known —
/// buffers/heap only mean something as deltas. A frame that stalls with no
/// mounts and a small scheduler pass has to be spending the time on something it
/// *created*, and this is where that shows up.
fn frame_line(s: &FrameSample, prev: Option<&FrameSample>) -> String {
    let d = prev.map_or(String::new(), |p| format!("  Δbuf {} Δbuf_mb {} Δheap {}MB", signed(s.buffers - p.buffers), signed(s.buffer_mb - p.buffer_mb), signed(s.heap_mb - p.heap_mb)));
    format!(
        "[{:>9}] dt {} = task {} + offthread {}  gpu {}  sched {}/{}{} (tick {})  draws {} tris {}k  buf {}  heap {}MB  mount +{}/-{} layouts {}{d}",
        n1(s.t),
        n1(s.dt_ms),
        n1(s.task_ms),
        n1((s.dt_ms - s.task_ms).max(0.0)),
        n1(s.gpu_ms),
        n1(s.sched_ms),
        n1(s.budget_ms),
        if s.settled { " still" } else { "" },
        n1(s.cpu.tick_ms),
        s.draw_calls,
        (s.triangles / 1000.0).round(),
        s.buffers,
        s.heap_mb,
        s.mounts.added,
        s.mounts.removed,
        s.mounts.layouts
    )
}

fn long_frame_line(l: &LongFrameRecord) -> String {
    let source = match l.source {
        LongFrameSource::Loaf => "loaf",
        LongFrameSource::Longtask => "longtask",
    };
    format!("[{:>9}] {source:<8} {}ms  script {}  render {}  blocking {}", n1(l.t), n1(l.duration_ms), n1(l.script_ms), n1(l.render_ms), n1(l.blocking_ms))
}

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

struct Log {
    baseline: Counters,
    logging: bool,
    lines: Vec<String>,
}

static LOG: Global<Log> = Global::new();

fn log() -> &'static mut Log {
    LOG.get(|| Log {
        baseline: counters(),
        logging: false,
        lines: Vec::new(),
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

fn log_task(rec: &TaskRecord) {
    log().lines.push(format!("[ledger] {}", task_line(rec)));
}

fn log_frame(s: &FrameSample) {
    if missed(s) || overran(s) {
        log().lines.push(format!("[ledger] {}", frame_line(s, None)));
    }
}

fn log_long_frame(l: &LongFrameRecord) {
    log().lines.push(format!("[ledger] {}", long_frame_line(l)));
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

/// The console lines the stream wrote since the last take.
pub fn take_log_lines() -> Vec<String> {
    core::mem::take(&mut log().lines)
}
