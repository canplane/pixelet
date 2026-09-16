// pxl_core/module/world/sched/log.rs

//! The ledger's text side — the two things the gauge panel can't do:
//!
//! - a **live console stream** of records as they land, so a stutter can be
//!   watched happening and correlated against whatever else is logging, rather
//!   than read off a panel that only shows the settled aggregate.
//! - a **text report** of the current rings, for pasting into a findings doc.
//!
//! Both are dev instruments and neither feeds back into scheduling. The live
//! stream thresholds frames deliberately: an unfiltered per-frame line would
//! bury everything else in the console within seconds, so only frames that
//! missed a vsync while moving, or overran the budget, are worth a line —
//! summary.rs's `missed` and `overran`, the same tests the report counts. The
//! console itself is the shell's: lines are handed over by `take_log_lines`.

use super::category::category_info;
use super::fetch_scheduler::{fetch_counts, fetch_credits};
use super::ledger::{
    ledger_frame_peak, ledger_frames_snapshot, ledger_long_frames_snapshot, ledger_recording_start, ledger_set_sink, ledger_snapshot, ledger_worst_frames, FrameSample, LedgerSink,
    LongFrameRecord, LongFrameSource, TaskRecord,
};
use super::summary::{census_rows, missed, overran, summarize_frames, summarize_tasks, tris_text, CensusRow, Spread, STARVED_MS, TILE_DECODED_MB};
use pxl_shared::spec::fetch::FetchFail;

use super::demand::Phases;
use super::tick::{tick_step_stats, tick_step_stats_reset, PHASES};
use crate::global::{now, Global};
use crate::kernel::kernel;
use crate::lib::env::config::{BUILD_ID, BUILD_NUMBER, BUILD_TIME};
use crate::module::world::billboard::frustum_budget::billboard_dropped;
use crate::module::world::chunk_node::{chunk_retention, RETAIN_BUDGET_BYTES};
use crate::module::world::poi::poi_badge::badge_atlas;
use crate::module::world::poi::poi_index::PoiIndexStats;
use crate::module::world::poi::poi_layer::PoiPlaceStats;

const RECENT_TASKS: usize = 20;
const MB: f64 = 1_048_576.0;

fn n1(n: f64) -> String {
    format!("{n:.1}")
}

fn share(part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        (part / whole * 100.0).round()
    } else {
        0.0
    }
}

fn census_text(rows: &[CensusRow]) -> String {
    rows.iter().map(|(tag, d, t)| format!("{tag} {}/{}", d.round(), tris_text(*t))).collect::<Vec<_>>().join("  ")
}

fn spread_text(s: &Spread) -> String {
    format!("p50 {}  p95 {}  max {}", n1(s.p50), n1(s.p95), n1(s.max))
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

/// One memory section as the shell's pool reports it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SectionStat {
    pub name: String,
    pub bytes: f64,
    pub live_bytes: f64,
    pub backing_used: f64,
    pub spills: f64,
    pub refusals: f64,
}

/// What the report reads of the shell: the memory, its sections, and whether
/// the browser has long-animation-frame entries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostReportStat {
    pub core_memory_bytes: f64,
    pub module_section_bytes: f64,
    pub module_heap_used: f64,
    pub reserved_bytes: f64,
    pub max_block_bytes: f64,
    pub sections: Vec<SectionStat>,
    pub loaf_supported: bool,
}

struct Log {
    baseline: Counters,
    logging: bool,
    lines: Vec<String>,
    host: HostReportStat,
}

static LOG: Global<Log> = Global::new();

fn log() -> &'static mut Log {
    LOG.get(|| Log {
        baseline: counters(),
        logging: false,
        lines: Vec::new(),
        host: HostReportStat::default(),
    })
}

/// The shell's numbers for the next report.
pub fn set_host_report_stat(stat: HostReportStat) {
    log().host = stat;
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

fn starved(ms: f64) -> &'static str {
    if ms > STARVED_MS {
        " STARVED"
    } else {
        ""
    }
}

/// Every registered step by mean ms per run, heaviest first.
fn steps_text() -> String {
    let mut rows: Vec<_> = tick_step_stats().iter().filter(|s| s.count > 0).collect();
    if rows.is_empty() {
        return "none".to_string();
    }
    rows.sort_by(|a, b| (b.ms / b.count as f64).total_cmp(&(a.ms / a.count as f64)));
    rows.iter().map(|s| format!("{}/{} {}/{}", PHASES[s.phase], s.name, n1(s.ms / s.count as f64), n1(s.max_ms))).collect::<Vec<_>>().join("  ")
}

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

fn frames_section(frames: &[FrameSample]) -> Vec<String> {
    let Some(f) = summarize_frames(frames) else { return vec!["frames n=0".to_string()] };
    let last = &f.last;
    let c = window_counters();
    let atlas = badge_atlas().stats();
    let index = PoiIndexStats::default();
    let place = PoiPlaceStats::default();
    let badge_count = 0;
    let retain = chunk_retention();
    let bands = c.bands_reused + c.bands_allocated;
    let host = &log().host;
    let census = kernel().sched.census;
    let counts = fetch_counts();

    vec![
        format!("frames n={}  over {}s  moving {}  still {}   display {} ms", f.n, n1(f.seconds), f.moving, f.n - f.moving, n1(last.frame_ms)),
        format!("  dt      {} ms   missed {} ({}% of moving frames)", spread_text(&f.dt), f.missed, share(f.missed as f64, f.moving as f64)),
        format!("  task    p50 {}  p95 {} ms   (main thread's own rAF task)", n1(f.task.p50), n1(f.task.p95)),
        format!("  offthd  p50 {}  p95 {} ms   (vsync/compositor/GPU/GC)", n1(f.offthread.p50), n1(f.offthread.p95)),
        format!("  gpu     p50 {}  p95 {} ms{}", n1(f.gpu.p50), n1(f.gpu.p95), if last.gpu_ms == 0.0 { "   (no renderer yet)" } else { "" }),
        format!(
            "  sched   p95 {}  max {} ms   overran {}   budget {}{}   phase p95 {}",
            n1(f.sched.p95),
            n1(f.sched.max),
            f.overran,
            n1(last.budget_ms),
            if last.settled { " (still)" } else { "" },
            f.phases.iter().map(|(k, v)| format!("{k} {}", n1(v.p95))).collect::<Vec<_>>().join("  ")
        ),
        format!("  cpu     tick p95 {} ms   (the kernel's tick inside the visible phase)", n1(f.tick.p95)),
        // Per step, so a phase that costs more than its steps should does not
        // hide which step is not yielding: a cooperative scheduler is only as
        // cooperative as its longest step.
        format!("  steps   {}   (ms per frame, max)", steps_text()),
        format!(
            "  gpu     draws {}  tris {}k  buffers {} {}MB  fill {:.1}Mpx",
            last.draw_calls,
            (last.triangles / 1000.0).round(),
            last.buffers,
            last.buffer_mb,
            last.px / 1e6
        ),
        format!("  targets depth {}MB  msaa {}MB  shadow {}MB  layers {}MB", last.textures_mb.depth, last.textures_mb.msaa, last.textures_mb.shadow, last.textures_mb.layers),
        format!("  by tag  mean {}   (draws/tris per frame)", census_text(&f.census)),
        format!("          last {}", census_text(&census_rows(&last.census))),
        format!("  mem     heap {}MB ({})  tiles {} (~{}MB)  poi {}", last.heap_mb, signed(f.heap_delta), last.tile_pages, last.tile_pages * TILE_DECODED_MB, last.poi_count),
        format!(
            "  place   poi {} ms over {} runs   hit {} -> cand {} -> kept {} (+{} absorbed, biggest {}{})   max since load {} ms, hit {}",
            n1(place.ms),
            c.place_runs,
            place.hits,
            place.candidates,
            place.kept,
            place.absorbed,
            place.biggest,
            if billboard_dropped() > 0 { format!(", DROPPED {}", billboard_dropped()) } else { String::new() },
            n1(place.max_ms),
            place.max_hits
        ),
        // Ask-to-commit and frames spanned: a placement is a job of the content
        // step, so this is its latency under the budget, and the max is the
        // starvation question.
        format!(
            "  wait    place {} ms over {} frames (max {} ms, {} frames)   index {} ms over {} frames (max {} ms, {} frames)",
            n1(place.wait_ms),
            place.frames,
            n1(place.max_wait_ms),
            place.max_frames,
            n1(index.wait_ms),
            index.frames,
            n1(index.max_wait_ms),
            index.max_frames
        ),
        format!(
            "  split   sync {} ms (max since load {})   badge {} measured / {} built {}ms total, {} cached",
            n1(place.sync_ms),
            n1(place.max_sync_ms),
            c.badge_metrics,
            c.badge_builds,
            n1(c.badge_ms),
            badge_count
        ),
        format!(
            "  atlas   {}px  {} entries  {} shelves  {}% used  {} repacks  {} rect uploads",
            atlas.size,
            atlas.entries,
            atlas.shelves,
            share(atlas.used_px as f64, atlas.size as f64 * atlas.size as f64),
            c.atlas_repacks,
            c.atlas_uploads
        ),
        format!("  faces   kept {} / {} ({}%)  runs culled {}/{}", c.faces_kept, c.faces_possible, share(c.faces_kept, c.faces_possible), c.runs_culled, c.runs_total),
        format!("  coarse  boxes {}  low fill {} ({}%; a threshold cull would have skipped these)", c.coarse_boxes, c.coarse_low_fill, share(c.coarse_low_fill, c.coarse_boxes)),
        format!(
            "  index   poi {} resident  {} rebuilds{}{}",
            index.indexed,
            c.index_rebuilds,
            if index.sweeping > 0 { format!("  sweeping {} units", index.sweeping) } else { String::new() },
            if index.dropped > 0 { format!("  DROPPED {}", index.dropped) } else { String::new() }
        ),
        // Idle inside the core reservation is the working budget, kept on
        // purpose; only what is reserved past it is ever handed back.
        format!(
            "  pool    live {}MB  idle {}MB  reserved {}MB (core {}MB kept)  hit {}%  biggest {}MB   window scans {} bakes {}",
            last.pool.live_mb,
            last.pool.idle_mb,
            (host.reserved_bytes / MB).round(),
            (host.core_memory_bytes / MB).round(),
            f.pool_hit_pct.map_or("-".to_string(), |p| p.to_string()),
            (host.max_block_bytes / MB).round(),
            f.work.0,
            f.work.1
        ),
        // Each section against its own range: the fixed one refuses past it,
        // the others spill to plain regions the collector can take back — a
        // spill or a refusal here is the memory map's own signal, per kind.
        format!(
            "  section module {:.1} heap of {}MB   {}   (live/cut of range)",
            host.module_heap_used / MB,
            (host.module_section_bytes / MB).round(),
            host.sections
                .iter()
                .map(|s| format!(
                    "{} {}/{} of {}MB{}{}",
                    s.name,
                    (s.live_bytes / MB).round(),
                    (s.backing_used / MB).round(),
                    (s.bytes / MB).round(),
                    if s.spills > 0.0 { format!(" SPILLED {}", s.spills) } else { String::new() },
                    if s.refusals > 0.0 { format!(" REFUSED {}", s.refusals) } else { String::new() }
                ))
                .collect::<Vec<_>>()
                .join("   ")
        ),
        format!("  fetch   queued {} running {} of {}", counts.queued, counts.running, fetch_credits()),
        // The builds run in the kernel under the tick; what they did this window.
        format!(
            "  build   {} direct  {} scans  {} bakes  {} refused a block{}",
            c.built_direct,
            c.built_scans,
            c.built_bakes,
            c.built_starved,
            if c.decor_dropped > 0.0 { format!("   DECOR DROPPED {}", c.decor_dropped) } else { String::new() }
        ),
        format!("  per     tick/bake {}ms   (workload-normalised; comparable across sessions)", f.tick_per_bake.map_or("-".to_string(), n1)),
        format!(
            "  rate    {} bakes/s  {} scans/s   while backlog {} bakes/s  {} scans/s over {}s",
            n1(f.per_s.1),
            n1(f.per_s.0),
            n1(f.backlog_per_s.1),
            n1(f.backlog_per_s.0),
            n1(f.backlog_s)
        ),
        format!("  bands   reused {} / {} ({}%)  ~1.31MB each; a miss borrows from the core pool", c.bands_reused, bands, share(c.bands_reused, bands)),
        format!(
            "  retain  nodes {}  runsets {} ({}MB)  tiers {}  decor {} steps {} instances   kept on-screen {}MB, off-screen {} / {}MB budget",
            retain.nodes,
            retain.run_sets,
            retain.run_set_mb,
            retain.tiers,
            retain.decor_steps,
            retain.decor_instances,
            (census.retained_on / MB).round(),
            (census.retained_off / MB).round(),
            (RETAIN_BUDGET_BYTES() / MB).round()
        ),
        format!(
            "  lod     chunks {}  in-frustum {}  pending {}  settling {}  blending {}  pressure {}",
            last.lod.chunks, last.lod.in_frustum, last.lod.pending, last.lod.settling, last.lod.blend_active, last.lod.pressure
        ),
        format!(
            "  wait    releases {} pending, oldest {} ms{}   on-screen oldest {} ms{}",
            last.lod.offscreen_pending,
            last.lod.oldest_release_ms.round(),
            starved(last.lod.oldest_release_ms),
            last.lod.oldest_visible_ms.round(),
            starved(last.lod.oldest_visible_ms)
        ),
        format!("  mounts  this frame +{}/-{} layouts {}   window total +{}/-{}", last.mounts.added, last.mounts.removed, last.mounts.layouts, f.mounts.0, f.mounts.1),
        format!("  queue   terrain {}  poi {}", last.q_terrain, last.q_poi),
    ]
}

/// The window's worst-seen values, including the LOD gauges the frame summary
/// only reads off the last frame.
fn peak_section() -> Vec<String> {
    let p = ledger_frame_peak();
    vec![
        "peak (window)".to_string(),
        format!("  dt {}  sched {}  tick {} ms", n1(p.dt_ms), n1(p.sched_ms), n1(p.tick_ms)),
        format!("  draws {}  tris {}k  heap {}MB  queue t{}/p{}", p.draw_calls, (p.triangles / 1000.0).round(), p.heap_mb, p.q_terrain, p.q_poi),
        format!("  lod chunks {}  in-frustum {}  pending {}  settling {}  blending {}  pressure {}", p.chunks, p.in_frustum, p.pending, p.settling, p.blend_active, p.pressure),
        format!(
            "  wait releases {} pending, oldest {} ms{}   on-screen oldest {} ms{}",
            p.offscreen_pending,
            p.oldest_release_ms.round(),
            starved(p.oldest_release_ms),
            p.oldest_visible_ms.round(),
            starved(p.oldest_visible_ms)
        ),
    ]
}

fn tasks_section(tasks: &[TaskRecord]) -> Vec<String> {
    if tasks.is_empty() {
        return vec!["tasks n=0".to_string()];
    }
    let mut lines = vec![format!("tasks n={}", tasks.len())];
    for t in summarize_tasks(tasks) {
        let bound = category_info(t.cat).map_or("?".to_string(), |c| c.bound.as_str().to_string());
        lines.push(format!(
            "  {:<9}{:<9}n={}   p50 {}  p95 {} ms   ok {}  ab {}  err {}{}",
            t.cat.as_str(),
            bound,
            t.n,
            n1(t.lat.p50),
            n1(t.lat.p95),
            t.ok,
            t.aborted,
            t.error,
            if t.reasons.is_empty() { String::new() } else { format!(" ({})", t.reasons.iter().map(|(r, n)| format!("{r} x{n}")).collect::<Vec<_>>().join(", ")) }
        ));
        if let Some(means) = &t.phase_mean {
            let means: Vec<String> = means.iter().map(|(k, v)| format!("{} {}", k.trim_end_matches("_ms"), if *k == "bytes" { v.round().to_string() } else { n1(*v) })).collect();
            lines.push(format!("  {:<18}mean {}", "", means.join("  ")));
        }
    }
    lines
}

/// The whole instrument as pasteable text: aggregate summary, then the raw tail
/// of each ring. Sized to land in a findings doc, not to be a trace — trace.rs
/// is the machine-readable path.
pub fn ledger_report_text() -> String {
    let frames = ledger_frames_snapshot();
    let tasks = ledger_snapshot();
    let longs = ledger_long_frames_snapshot();

    let mut out = vec![format!("=== sched/ledger @ t={}ms  build {} {} ({}) ===", n1(now()), BUILD_NUMBER(), BUILD_ID(), BUILD_TIME())];
    out.extend(frames_section(frames));
    if !frames.is_empty() {
        out.extend(peak_section());
    }
    out.extend(tasks_section(tasks));
    out.push(format!("long frames n={}{}", longs.len(), if log().host.loaf_supported { "" } else { " (LoAF unsupported; longtask only)" }));

    if !tasks.is_empty() {
        out.push(format!("--- last {} tasks ---", RECENT_TASKS.min(tasks.len())));
        for rec in &tasks[tasks.len().saturating_sub(RECENT_TASKS)..] {
            out.push(task_line(rec));
        }
    }
    let worst = ledger_worst_frames();
    if !worst.is_empty() {
        out.push(format!("--- worst {} frames in the window ---", worst.len()));
        for w in worst {
            out.push(frame_line(&w.sample, w.prev.as_ref()));
        }
    }
    if !longs.is_empty() {
        out.push("--- long frames ---".to_string());
        for l in longs {
            out.push(long_frame_line(l));
        }
    }
    out.join("\n")
}
