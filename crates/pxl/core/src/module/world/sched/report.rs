// pxl_core/module/world/sched/report.rs

//! The ledger as one document of numbers. The words that make it readable —
//! the column titles, the units, the asides that say what a line means — are
//! the shell's (apps/web/src/module/world/sched/ledger_report.ts), as
//! `strerror`'s are libc's and a trace event's are the reader of
//! `tracing/events/*/format`.
//!
//! What crosses is what was measured. Nothing here decides how it reads, so
//! nothing here holds a sentence.

use core::fmt::{self, Write};

use super::ledger::{
    ledger_frame_peak, ledger_frames_since, ledger_frames_snapshot, ledger_long_frames_since, ledger_long_frames_snapshot, ledger_recording_state, ledger_snapshot, ledger_tasks_since,
    ledger_worst_frames, FrameSample, LongFrameRecord, LongFrameSource, TaskRecord,
};
use super::log::{take_log_records, window_counters};
use super::summary::{summarize_frames, summarize_tasks, FrameSummary, Spread, TaskSummary};
use crate::global::now;
use crate::kernel::kernel;
use crate::lib::env::config::{BUILD_ID, BUILD_NUMBER, BUILD_TIME};
use crate::module::world::billboard::frustum_budget::billboard_dropped;
use crate::module::world::chunk_node::{chunk_retention, RETAIN_BUDGET_BYTES};
use crate::module::world::poi::poi_badge::badge_atlas;
use crate::module::world::poi::poi_index::PoiIndexStats;
use crate::module::world::poi::poi_layer::PoiPlaceStats;
use crate::module::world::sched::fetch_scheduler::{fetch_counts, fetch_credits};
use crate::module::world::sched::tick::{tick_step_stats, PHASES};

/// The tail of the task ring the report carries whole, newest last.
const RECENT_TASKS: usize = 20;

/// A number as JSON writes it: `null` for one that is not finite.
struct N(f64);

impl fmt::Display for N {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_finite() {
            write!(f, "{}", self.0)
        } else {
            f.write_str("null")
        }
    }
}

/// A value, or `null` for none.
struct Or<T>(Option<T>);

impl<T: fmt::Display> fmt::Display for Or<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(v) => v.fmt(f),
            None => f.write_str("null"),
        }
    }
}

/// A name as a JSON string. Every string this document carries is an
/// identifier the shell looks a word up by, never a word.
struct Q<'a>(&'a str);

impl fmt::Display for Q<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;
        for c in self.0.chars() {
            match c {
                '"' => f.write_str("\\\"")?,
                '\\' => f.write_str("\\\\")?,
                c if (c as u32) < 0x20 => write!(f, "\\u{:04x}", c as u32)?,
                c => f.write_char(c)?,
            }
        }
        f.write_char('"')
    }
}

fn spread<W: Write>(out: &mut W, s: &Spread) -> fmt::Result {
    write!(out, "{{\"p50\":{},\"p95\":{},\"max\":{}}}", N(s.p50), N(s.p95), N(s.max))
}

fn frame<W: Write>(out: &mut W, s: &FrameSample) -> fmt::Result {
    write!(
        out,
        "{{\"t\":{},\"dt_ms\":{},\"frame_ms\":{},\"settled\":{},\"sched_ms\":{},\"budget_ms\":{},\"task_ms\":{},\"gpu_ms\":{},\"tick_ms\":{},",
        N(s.t),
        N(s.dt_ms),
        N(s.frame_ms),
        s.settled,
        N(s.sched_ms),
        N(s.budget_ms),
        N(s.task_ms),
        N(s.gpu_ms),
        N(s.cpu.tick_ms)
    )?;
    out.write_str("\"phases\":[")?;
    for (i, v) in s.phases.iter().enumerate() {
        write!(out, "{}{}", if i > 0 { "," } else { "" }, N(*v))?;
    }
    write!(
        out,
        "],\"draw_calls\":{},\"triangles\":{},\"buffers\":{},\"buffer_mb\":{},\"px\":{},\"heap_mb\":{},\"tile_pages\":{},\"poi_count\":{},",
        N(s.draw_calls),
        N(s.triangles),
        N(s.buffers),
        N(s.buffer_mb),
        N(s.px),
        N(s.heap_mb),
        N(s.tile_pages),
        N(s.poi_count)
    )?;
    write!(
        out,
        "\"textures_mb\":{{\"depth\":{},\"msaa\":{},\"shadow\":{},\"layers\":{}}},",
        N(s.textures_mb.depth),
        N(s.textures_mb.msaa),
        N(s.textures_mb.shadow),
        N(s.textures_mb.layers)
    )?;
    out.write_str("\"census\":[")?;
    for (i, (d, t)) in s.census.iter().enumerate() {
        write!(out, "{}[{},{}]", if i > 0 { "," } else { "" }, N(*d), N(*t))?;
    }
    write!(
        out,
        "],\"pool\":{{\"live_mb\":{},\"idle_mb\":{},\"hits\":{},\"misses\":{}}},",
        N(s.pool.live_mb),
        N(s.pool.idle_mb),
        N(s.pool.hits),
        N(s.pool.misses)
    )?;
    write!(out, "\"mounts\":{{\"added\":{},\"removed\":{},\"layouts\":{}}},", N(s.mounts.added), N(s.mounts.removed), N(s.mounts.layouts))?;
    write!(
        out,
        "\"lod\":{{\"chunks\":{},\"in_frustum\":{},\"pending\":{},\"settling\":{},\"blend_active\":{},\"offscreen_pending\":{},\"oldest_release_ms\":{},\"oldest_visible_ms\":{},\"pressure\":{}}},",
        N(s.lod.chunks),
        N(s.lod.in_frustum),
        N(s.lod.pending),
        N(s.lod.settling),
        N(s.lod.blend_active),
        N(s.lod.offscreen_pending),
        N(s.lod.oldest_release_ms),
        N(s.lod.oldest_visible_ms),
        N(s.lod.pressure)
    )?;
    write!(out, "\"q_terrain\":{},\"q_poi\":{}}}", N(s.q_terrain), N(s.q_poi))
}

fn task<W: Write>(out: &mut W, r: &TaskRecord) -> fmt::Result {
    write!(
        out,
        "{{\"category\":{},\"subject\":{},\"priority\":{},\"enqueue_ms\":{},\"start_ms\":{},\"end_ms\":{},\"transport_ms\":{},\"outcome\":{},\"fail\":{},\"status\":{},",
        Q(r.key.category.as_str()),
        r.key.subject,
        N(r.priority),
        N(r.enqueue_ms),
        Or(r.start_ms.map(N)),
        N(r.end_ms),
        N(r.transport_ms),
        Q(r.outcome.as_str()),
        r.fail.fail as u32,
        r.fail.status
    )?;
    out.write_str("\"phases\":[")?;
    let mut first = true;
    for (k, v) in r.phases.iter().flat_map(|p| p.iter()) {
        write!(out, "{}[{},{}]", if first { "" } else { "," }, Q(k), N(v))?;
        first = false;
    }
    out.write_str("]}")
}

fn summary<W: Write>(out: &mut W, t: &TaskSummary) -> fmt::Result {
    write!(
        out,
        "{{\"category\":{},\"n\":{},\"ok\":{},\"aborted\":{},\"error\":{},\"lat\":",
        Q(t.cat.as_str()),
        t.n,
        t.ok,
        t.aborted,
        t.error
    )?;
    spread(out, &t.lat)?;
    out.write_str(",\"reasons\":[")?;
    for (i, (r, n)) in t.reasons.iter().enumerate() {
        write!(out, "{}{{\"fail\":{},\"status\":{},\"n\":{}}}", if i > 0 { "," } else { "" }, r.fail as u32, r.status, n)?;
    }
    out.write_str("],\"phase_mean\":")?;
    match &t.phase_mean {
        None => out.write_str("null")?,
        Some(means) => {
            out.write_char('[')?;
            for (i, (k, v)) in means.iter().enumerate() {
                write!(out, "{}[{},{}]", if i > 0 { "," } else { "" }, Q(k), N(*v))?;
            }
            out.write_char(']')?;
        }
    }
    out.write_char('}')
}

fn long_frame<W: Write>(out: &mut W, l: &LongFrameRecord) -> fmt::Result {
    write!(
        out,
        "{{\"source\":{},\"t\":{},\"duration_ms\":{},\"blocking_ms\":{},\"script_ms\":{},\"render_ms\":{}}}",
        Q(match l.source {
            LongFrameSource::Loaf => "loaf",
            LongFrameSource::Longtask => "longtask",
        }),
        N(l.t),
        N(l.duration_ms),
        N(l.blocking_ms),
        N(l.script_ms),
        N(l.render_ms)
    )
}

fn frames_summary<W: Write>(out: &mut W, f: &FrameSummary) -> fmt::Result {
    write!(out, "{{\"n\":{},\"seconds\":{},\"moving\":{},\"missed\":{},\"overran\":{},", f.n, N(f.seconds), f.moving, f.missed, f.overran)?;
    for (name, s) in [("dt", &f.dt), ("task", &f.task), ("offthread", &f.offthread), ("gpu", &f.gpu), ("sched", &f.sched), ("tick", &f.tick)] {
        write!(out, "{}:", Q(name))?;
        spread(out, s)?;
        out.write_char(',')?;
    }
    out.write_str("\"phases\":[")?;
    for (i, (k, s)) in f.phases.iter().enumerate() {
        write!(out, "{}{{\"name\":{},\"spread\":", if i > 0 { "," } else { "" }, Q(k))?;
        spread(out, s)?;
        out.write_char('}')?;
    }
    write!(
        out,
        "],\"work\":[{},{}],\"per_s\":[{},{}],\"backlog_s\":{},\"backlog_per_s\":[{},{}],\"tick_per_bake\":{},\"mounts\":[{},{}],\"pool_hit_pct\":{},\"heap_delta\":{},",
        N(f.work.0),
        N(f.work.1),
        N(f.per_s.0),
        N(f.per_s.1),
        N(f.backlog_s),
        N(f.backlog_per_s.0),
        N(f.backlog_per_s.1),
        Or(f.tick_per_bake.map(N)),
        N(f.mounts.0),
        N(f.mounts.1),
        Or(f.pool_hit_pct.map(N)),
        N(f.heap_delta)
    )?;
    out.write_str("\"census\":[")?;
    for (i, (tag, d, t)) in f.census.iter().enumerate() {
        write!(out, "{}{{\"tag\":{},\"draws\":{},\"tris\":{}}}", if i > 0 { "," } else { "" }, Q(tag), N(*d), N(*t))?;
    }
    out.write_str("],\"last\":")?;
    frame(out, &f.last)?;
    out.write_char('}')
}

/// The gauge panel's numbers, over the last `window_ms` rather than the whole
/// recording.
///
/// The window is the point of this being its own document. The panel repaints
/// twice a second, and folding every frame since load on each repaint grew
/// into a sixty-millisecond task two minutes in — the panel became the stall
/// it was drawing. The report still covers the whole window; this covers what
/// a person watching is actually looking at.
pub fn ledger_gauge_json(window_ms: f64) -> String {
    let mut out = String::new();
    let _ = gauge(&mut out, window_ms);
    out
}

fn gauge<W: Write>(out: &mut W, window_ms: f64) -> fmt::Result {
    let since = now() - window_ms;
    let rec = ledger_recording_state();
    let c = window_counters();
    let atlas = badge_atlas().stats();
    // As report(): POI is off until its index lives with its pages, and the
    // panel keeps its rows rather than losing the place they come back to.
    let index = PoiIndexStats::default();
    let place = PoiPlaceStats::default();

    write!(
        out,
        "{{\"rec\":{{\"recording\":{},\"elapsed_ms\":{},\"tasks\":{},\"frames\":{},\"long_frames\":{},\"truncated\":{}}},",
        rec.recording,
        N(rec.elapsed_ms),
        rec.tasks,
        rec.frames,
        rec.long_frames,
        rec.truncated
    )?;

    out.write_str("\"frames\":")?;
    match summarize_frames(ledger_frames_since(since)) {
        Some(f) => frames_summary(out, &f)?,
        None => out.write_str("null")?,
    }

    out.write_str(",\"tasks\":[")?;
    for (i, t) in summarize_tasks(ledger_tasks_since(since)).iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        summary(out, t)?;
    }

    write!(out, "],\"faces\":[{},{}],", N(c.faces_kept), N(c.faces_possible))?;
    write!(
        out,
        "\"atlas\":{{\"size\":{},\"entries\":{},\"shelves\":{},\"used_px\":{}}},",
        atlas.size, atlas.entries, atlas.shelves, atlas.used_px
    )?;
    write!(
        out,
        "\"poi\":{{\"ms\":{},\"frames\":{},\"max_frames\":{},\"hits\":{},\"candidates\":{},\"kept\":{},\"indexed\":{},\"dropped\":{}}},",
        N(place.ms),
        place.frames,
        place.max_frames,
        place.hits,
        place.candidates,
        place.kept,
        index.indexed,
        index.dropped
    )?;

    out.write_str("\"longs\":[")?;
    for (i, l) in ledger_long_frames_since(since).iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        long_frame(out, l)?;
    }
    out.write_str("]}")
}

/// What the live stream kept since the shell last came for it, as the same
/// records the report carries — so the shell's formatters draw a streamed task
/// and a reported one with the one piece of code (ledger_report.ts).
pub fn ledger_log_json() -> String {
    let (tasks, frames, longs) = take_log_records();
    let mut out = String::new();
    let _ = log_records(&mut out, &tasks, &frames, &longs);
    out
}

fn log_records<W: Write>(out: &mut W, tasks: &[TaskRecord], frames: &[FrameSample], longs: &[LongFrameRecord]) -> fmt::Result {
    out.write_str("{\"tasks\":[")?;
    for (i, r) in tasks.iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        task(out, r)?;
    }
    out.write_str("],\"frames\":[")?;
    for (i, f) in frames.iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        frame(out, f)?;
    }
    out.write_str("],\"longs\":[")?;
    for (i, l) in longs.iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        long_frame(out, l)?;
    }
    out.write_str("]}")
}

/// The whole instrument as one document of numbers, for the shell to read.
pub fn ledger_report_json() -> String {
    let mut out = String::new();
    let _ = report(&mut out);
    out
}

fn report<W: Write>(out: &mut W) -> fmt::Result {
    let frames = ledger_frames_snapshot();
    let tasks = ledger_snapshot();
    let longs = ledger_long_frames_snapshot();
    let c = window_counters();
    let atlas = badge_atlas().stats();
    let retain = chunk_retention();
    let census = kernel().sched.census;
    let counts = fetch_counts();
    // POI is off until its index lives with its pages (issue/BACKLOG.md), so
    // these read zero rather than being left out: the report keeps their place.
    let index = PoiIndexStats::default();
    let place = PoiPlaceStats::default();

    write!(
        out,
        "{{\"t\":{},\"build\":{{\"number\":{},\"id\":{},\"time\":{}}},",
        N(now()),
        Q(BUILD_NUMBER()),
        Q(BUILD_ID()),
        Q(BUILD_TIME())
    )?;

    out.write_str("\"frames\":")?;
    match summarize_frames(frames) {
        Some(f) => frames_summary(out, &f)?,
        None => out.write_str("null")?,
    }

    write!(
        out,
        ",\"counters\":{{\"faces_kept\":{},\"faces_possible\":{},\"runs_culled\":{},\"runs_total\":{},\"coarse_boxes\":{},\"coarse_low_fill\":{},\"bands_reused\":{},\"bands_allocated\":{},\"built_direct\":{},\"built_scans\":{},\"built_bakes\":{},\"built_starved\":{},\"decor_dropped\":{},\"badge_metrics\":{},\"badge_builds\":{},\"badge_ms\":{},\"place_runs\":{},\"atlas_repacks\":{},\"atlas_uploads\":{},\"index_rebuilds\":{}}},",
        N(c.faces_kept),
        N(c.faces_possible),
        N(c.runs_culled),
        N(c.runs_total),
        N(c.coarse_boxes),
        N(c.coarse_low_fill),
        N(c.bands_reused),
        N(c.bands_allocated),
        N(c.built_direct),
        N(c.built_scans),
        N(c.built_bakes),
        N(c.built_starved),
        N(c.decor_dropped),
        N(c.badge_metrics),
        N(c.badge_builds),
        N(c.badge_ms),
        N(c.place_runs),
        N(c.atlas_repacks),
        N(c.atlas_uploads),
        N(c.index_rebuilds)
    )?;

    write!(
        out,
        "\"atlas\":{{\"size\":{},\"entries\":{},\"shelves\":{},\"used_px\":{}}},",
        atlas.size, atlas.entries, atlas.shelves, atlas.used_px
    )?;
    write!(
        out,
        "\"retain\":{{\"nodes\":{},\"run_sets\":{},\"run_set_mb\":{},\"tiers\":{},\"decor_steps\":{},\"decor_instances\":{},\"on_bytes\":{},\"off_bytes\":{},\"budget_bytes\":{}}},",
        retain.nodes,
        retain.run_sets,
        retain.run_set_mb,
        retain.tiers,
        retain.decor_steps,
        retain.decor_instances,
        N(census.retained_on),
        N(census.retained_off),
        N(RETAIN_BUDGET_BYTES())
    )?;
    write!(out, "\"fetch\":{{\"queued\":{},\"running\":{},\"credits\":{}}},", counts.queued, counts.running, fetch_credits())?;
    write!(
        out,
        "\"poi\":{{\"badge_count\":0,\"dropped\":{},\"place\":{{\"ms\":{},\"hits\":{},\"candidates\":{},\"kept\":{},\"absorbed\":{},\"biggest\":{},\"max_ms\":{},\"max_hits\":{},\"wait_ms\":{},\"frames\":{},\"max_wait_ms\":{},\"max_frames\":{},\"sync_ms\":{},\"max_sync_ms\":{}}},\"index\":{{\"indexed\":{},\"sweeping\":{},\"dropped\":{},\"wait_ms\":{},\"frames\":{},\"max_wait_ms\":{},\"max_frames\":{}}}}},",
        billboard_dropped(),
        N(place.ms),
        place.hits,
        place.candidates,
        place.kept,
        place.absorbed,
        place.biggest,
        N(place.max_ms),
        place.max_hits,
        N(place.wait_ms),
        place.frames,
        N(place.max_wait_ms),
        place.max_frames,
        N(place.sync_ms),
        N(place.max_sync_ms),
        index.indexed,
        index.sweeping,
        index.dropped,
        N(index.wait_ms),
        index.frames,
        N(index.max_wait_ms),
        index.max_frames
    )?;

    // Per step, so a phase that costs more than its steps should does not hide
    // which step is not yielding: a cooperative scheduler is only as
    // cooperative as its longest step.
    out.write_str("\"steps\":[")?;
    for (i, s) in tick_step_stats().iter().filter(|s| s.count > 0).enumerate() {
        write!(
            out,
            "{}{{\"phase\":{},\"name\":{},\"ms\":{},\"count\":{},\"max_ms\":{}}}",
            if i > 0 { "," } else { "" },
            Q(PHASES[s.phase]),
            Q(s.name),
            N(s.ms),
            s.count,
            N(s.max_ms)
        )?;
    }
    out.write_str("],")?;

    out.write_str("\"peak\":")?;
    if frames.is_empty() {
        out.write_str("null")?;
    } else {
        let p = ledger_frame_peak();
        write!(
            out,
            "{{\"dt_ms\":{},\"sched_ms\":{},\"tick_ms\":{},\"draw_calls\":{},\"triangles\":{},\"heap_mb\":{},\"q_terrain\":{},\"q_poi\":{},\"chunks\":{},\"in_frustum\":{},\"pending\":{},\"settling\":{},\"blend_active\":{},\"pressure\":{},\"offscreen_pending\":{},\"oldest_release_ms\":{},\"oldest_visible_ms\":{}}}",
            N(p.dt_ms),
            N(p.sched_ms),
            N(p.tick_ms),
            N(p.draw_calls),
            N(p.triangles),
            N(p.heap_mb),
            N(p.q_terrain),
            N(p.q_poi),
            N(p.chunks),
            N(p.in_frustum),
            N(p.pending),
            N(p.settling),
            N(p.blend_active),
            N(p.pressure),
            N(p.offscreen_pending),
            N(p.oldest_release_ms),
            N(p.oldest_visible_ms)
        )?;
    }

    write!(out, ",\"task_n\":{},\"tasks\":[", tasks.len())?;
    for (i, t) in summarize_tasks(tasks).iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        summary(out, t)?;
    }

    out.write_str("],\"recent\":[")?;
    for (i, r) in tasks[tasks.len().saturating_sub(RECENT_TASKS)..].iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        task(out, r)?;
    }

    out.write_str("],\"worst\":[")?;
    for (i, w) in ledger_worst_frames().iter().enumerate() {
        write!(out, "{}{{\"sample\":", if i > 0 { "," } else { "" })?;
        frame(out, &w.sample)?;
        out.write_str(",\"prev\":")?;
        match &w.prev {
            Some(p) => frame(out, p)?,
            None => out.write_str("null")?,
        }
        out.write_char('}')?;
    }

    out.write_str("],\"longs\":[")?;
    for (i, l) in longs.iter().enumerate() {
        if i > 0 {
            out.write_char(',')?;
        }
        long_frame(out, l)?;
    }
    out.write_str("]}")?;
    Ok(())
}
