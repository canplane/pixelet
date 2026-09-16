// pxl_core/module/world/sched/trace.rs

//! Chrome Trace Event Format dump of the ledger rings, openable in Perfetto
//! (ui.perfetto.dev) or chrome://tracing. Aggregates the same numbers the dev
//! overlay shows, but on one timeline — the "trace, not just aggregates" point
//! in issue/20260909-cooperative-scheduler-and-task-ledger.md: p95 says a
//! category is slow, the timeline shows it's slow *because* it's contending
//! with a fetch burst that the same pan kicked off.
//!
//! Dev instrument. Recording covers the whole window (ledger.rs), so a trace
//! is routinely tens of MB — the shell saves it as a file.

use pxl_shared::spec::fetch::FetchFail;
use super::ledger::{ledger_frames_snapshot, ledger_long_frames_snapshot, ledger_snapshot, ledger_worst_frames, FrameSample};
use crate::lib::env::config::{BUILD_ID, BUILD_NUMBER, BUILD_TIME};

const PID: u32 = 1;
const TID_FRAMES: u32 = 1;
const TID_LONG: u32 = 2;
const TID_WORST: u32 = 3;
// task threads are assigned per category from here up
const TID_TASK_BASE: u32 = 10;

fn ms_to_us(ms: f64) -> f64 {
    (ms * 1000.0 + 0.5).floor()
}

/// `+n.toFixed(d)`, as JSON.
fn fixed(n: f64, d: i32) -> String {
    let k = 10f64.powi(d);
    json_number((n * k).round() / k)
}

fn json_number(n: f64) -> String {
    if !n.is_finite() {
        return "null".to_string();
    }
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn object(fields: &[(String, String)]) -> String {
    let body: Vec<String> = fields.iter().map(|(k, v)| format!("{}:{}", json_string(k), v)).collect();
    format!("{{{}}}", body.join(","))
}

fn field(k: &str, v: String) -> (String, String) {
    (k.to_string(), v)
}

#[allow(clippy::too_many_arguments)]
fn event(name: &str, cat: &str, ph: &str, ts: f64, dur: Option<f64>, tid: u32, args: Option<String>) -> String {
    let mut fields = vec![field("name", json_string(name)), field("cat", json_string(cat)), field("ph", json_string(ph)), field("ts", json_number(ts))];
    if let Some(dur) = dur {
        fields.push(field("dur", json_number(dur)));
    }
    fields.push(field("pid", json_number(PID as f64)));
    fields.push(field("tid", json_number(tid as f64)));
    if let Some(args) = args {
        fields.push(field("args", args));
    }
    object(&fields)
}

fn thread_name(tid: u32, name: &str) -> String {
    event("thread_name", "__metadata", "M", 0.0, None, tid, Some(object(&[field("name", json_string(name))])))
}

/// One frame's numbers as trace args — the counter track and the worst-frame
/// spans both carry them, so the two never disagree on what a frame was.
fn frame_args(f: &FrameSample) -> Vec<(String, String)> {
    let mut args = vec![
        field("dt_ms", fixed(f.dt_ms, 2)),
        field("task_ms", fixed(f.task_ms, 2)),
        // everything the main thread's own task did not account for: vsync
        // wait, compositor, GPU, GC between frames.
        field("offthread_ms", fixed((f.dt_ms - f.task_ms).max(0.0), 2)),
        field("gpu_ms", fixed(f.gpu_ms, 2)),
        field("sched_ms", fixed(f.sched_ms, 2)),
        field("budget_ms", fixed(f.budget_ms, 2)),
        field("frame_ms", fixed(f.frame_ms, 2)),
        field("settled", json_number(if f.settled { 1.0 } else { 0.0 })),
    ];
    for (k, v) in super::tick::PHASES.iter().zip(f.phases) {
        args.push((format!("phase_{k}_ms"), fixed(v, 2)));
    }
    args.push(field("tick_ms", fixed(f.cpu.tick_ms, 2)));
    args.push(field("triangles", json_number(f.triangles)));
    for (tag, (_, tris)) in super::ledger::CENSUS_TAGS.iter().zip(&f.census) {
        args.push((format!("tris_{tag}"), json_number(*tris)));
    }
    let t = f.textures_mb;
    args.extend([
        field("buffers", json_number(f.buffers)),
        field("buffer_mb", json_number(f.buffer_mb)),
        field("targets_mb", fixed(t.depth + t.msaa + t.shadow + t.layers, 1)),
        field("mounted", json_number(f.mounts.added)),
        field("unmounted", json_number(f.mounts.removed)),
        field("layouts", json_number(f.mounts.layouts)),
        field("heap_mb", json_number(f.heap_mb)),
        field("pool_live_mb", json_number(f.pool.live_mb)),
        field("pool_idle_mb", json_number(f.pool.idle_mb)),
        field("pool_hits", json_number(f.pool.hits)),
        field("pool_misses", json_number(f.pool.misses)),
        field("scans", json_number(f.work.scans)),
        field("bakes", json_number(f.work.bakes)),
        field("tile_pages", json_number(f.tile_pages)),
        field("poi_count", json_number(f.poi_count)),
        field("lod_pending", json_number(f.lod.pending)),
        field("lod_in_frustum", json_number(f.lod.in_frustum)),
        field("lod_offscreen_pending", json_number(f.lod.offscreen_pending)),
        field("lod_oldest_release_ms", json_number(f.lod.oldest_release_ms.round())),
        field("lod_oldest_visible_ms", json_number(f.lod.oldest_visible_ms.round())),
        field("draw_calls", json_number(f.draw_calls)),
        field("fill_mpx", fixed(f.px / 1e6, 2)),
        field("q_terrain", json_number(f.q_terrain)),
        field("q_poi", json_number(f.q_poi)),
    ]);
    args
}

/// The trace as a JSON string.
pub fn ledger_trace_json() -> String {
    let mut events: Vec<String> = Vec::new();
    let tasks = ledger_snapshot();
    let frames = ledger_frames_snapshot();
    let longs = ledger_long_frames_snapshot();

    // ---- task spans, one thread per category ----
    let mut cat_tid: Vec<(String, u32)> = Vec::new();
    for rec in tasks {
        let tid = match cat_tid.iter().find(|(c, _)| *c == rec.category) {
            Some((_, tid)) => *tid,
            None => {
                let tid = TID_TASK_BASE + cat_tid.len() as u32;
                cat_tid.push((rec.category.to_string(), tid));
                events.push(thread_name(tid, &format!("task: {}", rec.category)));
                tid
            }
        };
        let mut args = vec![
            field("outcome", json_string(rec.outcome.as_str())),
            field("priority", json_number(rec.priority)),
            field("queued_ms", rec.start_ms.map_or("null".to_string(), |s| fixed(s - rec.enqueue_ms, 1))),
            field("ran_ms", rec.start_ms.map_or("null".to_string(), |s| fixed(rec.end_ms - s, 1))),
            field("transport_ms", json_number(rec.transport_ms)),
        ];
        if rec.fail.fail != FetchFail::None {
            args.push(field("fail", json_string(rec.fail.fail.as_str())));
            if rec.fail.status != 0 {
                args.push(field("status", json_number(rec.fail.status as f64)));
            }
        }
        for (k, v) in rec.phases.iter().flat_map(|p| p.iter()) {
            args.push((k.to_string(), json_number(v)));
        }
        events.push(event(
            &rec.key,
            &format!("task,{}", rec.outcome.as_str()),
            "X",
            ms_to_us(rec.enqueue_ms),
            Some(ms_to_us((rec.end_ms - rec.enqueue_ms).max(0.0))),
            tid,
            Some(object(&args)),
        ));
    }

    // ---- per-frame counters (Perfetto graphs these) ----
    if !frames.is_empty() {
        events.push(thread_name(TID_FRAMES, "frames"));
        for f in frames {
            let ts = ms_to_us(f.t);
            events.push(event("frame", "frame", "C", ts, None, TID_FRAMES, Some(object(&frame_args(f)))));
            // also a duration bar for the scheduler pass, so a slow frame is
            // visible as width, not only as a counter spike
            events.push(event("sched pass", "frame", "X", ts - ms_to_us(f.sched_ms), Some(ms_to_us(f.sched_ms)), TID_FRAMES, None));
        }
    }

    // ---- worst frames of the window ----
    // Already on the counter track above; repeated as labelled spans so a
    // stall can be found without scrubbing, at its real timestamp.
    let worst = ledger_worst_frames();
    if !worst.is_empty() {
        events.push(thread_name(TID_WORST, "worst frames"));
        for w in worst {
            let f = &w.sample;
            let mut args = frame_args(f);
            args.push(field("d_buffers", w.prev.as_ref().map_or("null".to_string(), |p| json_number(f.buffers - p.buffers))));
            args.push(field("d_heap_mb", w.prev.as_ref().map_or("null".to_string(), |p| json_number(f.heap_mb - p.heap_mb))));
            events.push(event(&format!("stall {:.0}ms", f.dt_ms), "worst", "X", ms_to_us(f.t - f.dt_ms), Some(ms_to_us(f.dt_ms)), TID_WORST, Some(object(&args))));
        }
    }

    // ---- long animation frames ----
    if !longs.is_empty() {
        events.push(thread_name(TID_LONG, "LoAF"));
        for l in longs {
            let args = object(&[field("blocking_ms", fixed(l.blocking_ms, 1)), field("script_ms", fixed(l.script_ms, 1)), field("render_ms", fixed(l.render_ms, 1))]);
            events.push(event(&format!("long frame {:.0}ms", l.duration_ms), "loaf", "X", ms_to_us(l.t), Some(ms_to_us(l.duration_ms)), TID_LONG, Some(args)));
        }
    }

    // Chrome's trace format keeps a top-level `metadata` object; a trace read
    // back later names the build it came from.
    let metadata = object(&[field("build_number", json_string(BUILD_NUMBER())), field("build_id", json_string(BUILD_ID())), field("build_time", json_string(BUILD_TIME()))]);
    format!("{{\"traceEvents\":[{}],\"displayTimeUnit\":\"ms\",\"metadata\":{}}}", events.join(","), metadata)
}
