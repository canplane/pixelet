// @pxl/web/dev/ledger/trace.ts

/**
 * The ledger window as Chrome's Trace Event Format, saved as a file openable
 * at ui.perfetto.dev or chrome://tracing. Aggregates the same numbers the dev
 * overlay shows, but on one timeline — the "trace, not just aggregates" point
 * in issue/20260909-cooperative-scheduler-and-task-ledger.md: p95 says a
 * category is slow, the timeline shows it's slow *because* it's contending
 * with a fetch burst that the same pan kicked off.
 *
 * Built here from the kernel's rings read in place (view.ts) and the
 * shell's own long frames, so the tens of megabytes a window's trace runs to
 * are this side's to allocate and never the kernel heap's. A download rather
 * than the clipboard: the clipboard handles that size badly, and Perfetto
 * wants a file anyway. Wired to `window` in dev (overlay.tsx) so it is
 * reachable from the console.
 */

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/config/deploy.ts";

import { fail_text, type Frame } from "./report.ts";
import { CENSUS_TAGS, PHASE_NAMES } from "./spec.ts";
import { ledger_rings_read } from "./view.ts";
import { long_frames_since } from "./long_frame.ts";

const PID = 1;
const TID_FRAMES = 1;
const TID_LONG = 2;
const TID_WORST = 3;
/** Task threads are assigned per category from here up. */
const TID_TASK_BASE = 10;

type Args = Record<string, number | string | null>;
type TraceEvent = { name: string; cat: string; ph: string; ts: number; dur?: number; pid: number; tid: number; args?: Args };

const us = (ms: number) => Math.round(ms * 1000);
const fixed = (n: number, d: number) => +n.toFixed(d);

function event(name: string, cat: string, ph: string, ts: number, dur: number | null, tid: number, args?: Args): TraceEvent {
  return { name, cat, ph, ts, ...(dur === null ? {} : { dur }), pid: PID, tid, ...(args ? { args } : {}) };
}

const thread_name = (tid: number, name: string) => event("thread_name", "__metadata", "M", 0, null, tid, { name });

/** One frame's numbers as trace args — the counter track and the worst-frame
 * spans both carry them, so the two never disagree on what a frame was. */
function frame_args(f: Frame): Args {
  const args: Args = {
    dt_ms: fixed(f.dt_ms, 2),
    task_ms: fixed(f.task_ms, 2),
    // Everything the kernel's turn did not account for: vsync wait,
    // compositor, GPU, GC between frames, the shell's glue.
    offthread_ms: fixed(Math.max(0, f.dt_ms - f.task_ms), 2),
    gpu_ms: fixed(f.gpu_ms, 2),
    sched_ms: fixed(f.sched_ms, 2),
    budget_ms: fixed(f.budget_ms, 2),
    frame_ms: fixed(f.frame_ms, 2),
    settled: f.settled ? 1 : 0,
  };
  PHASE_NAMES.forEach((k, i) => (args[`phase_${k}_ms`] = fixed(f.phases[i], 2)));
  args.tick_ms = fixed(f.tick_ms, 2);
  args.triangles = f.triangles;
  CENSUS_TAGS.forEach((tag, i) => (args[`tris_${tag}`] = f.census[i][1]));
  const t = f.textures_mb;
  Object.assign(args, {
    buffers: f.buffers,
    buffer_mb: f.buffer_mb,
    targets_mb: fixed(t.depth + t.msaa + t.shadow + t.layers, 1),
    mounted: f.mounts.added,
    unmounted: f.mounts.removed,
    layouts: f.mounts.layouts,
    pool_used_mb: f.pool.used_mb,
    pool_live_mb: f.pool.live_mb,
    pool_range_mb: f.pool.range_mb,
    pool_refusals: f.pool.refusals,
    pool_largest_free_mb: fixed(f.pool.largest_free_mb, 2),
    ring_cq_peak: f.rings.cq_peak,
    ring_spill_peak: f.rings.spill_peak,
    tile_pages: f.tile_pages,
    poi_count: f.poi_count,
    lod_pending: f.lod.pending,
    lod_in_frustum: f.lod.in_frustum,
    lod_offscreen_pending: f.lod.offscreen_pending,
    lod_oldest_release_ms: Math.round(f.lod.oldest_release_ms),
    lod_oldest_visible_ms: Math.round(f.lod.oldest_visible_ms),
    draw_calls: f.draw_calls,
    fill_mpx: fixed(f.px / 1e6, 2),
    q_terrain: f.q_terrain,
    q_poi: f.q_poi,
  });
  return args;
}

/** The trace as a JSON string. */
export function ledger_trace_json(): string {
  const { frames, tasks, worst } = ledger_rings_read();
  const longs = long_frames_since(-Infinity);
  const events: TraceEvent[] = [];

  // ---- task spans, one thread per category ----
  const cat_tid = new Map<string, number>();
  for (const r of tasks) {
    let tid = cat_tid.get(r.category);
    if (tid === undefined) {
      tid = TID_TASK_BASE + cat_tid.size;
      cat_tid.set(r.category, tid);
      events.push(thread_name(tid, `task: ${r.category}`));
    }
    const args: Args = {
      outcome: r.outcome,
      priority: r.priority,
      queued_ms: r.start_ms === null ? null : fixed(r.start_ms - r.enqueue_ms, 1),
      ran_ms: r.start_ms === null ? null : fixed(r.end_ms - r.start_ms, 1),
      transport_ms: r.transport_ms,
    };
    if (r.fail !== 0) {
      args.fail = fail_text(r.fail, r.status);
      if (r.status !== 0) args.status = r.status;
    }
    for (const [k, v] of r.phases) args[k] = v;
    events.push(event(`${r.category}:${r.subject}`, `task,${r.outcome}`, "X", us(r.enqueue_ms), us(Math.max(0, r.end_ms - r.enqueue_ms)), tid, args));
  }

  // ---- per-frame counters (Perfetto graphs these) ----
  if (frames.length > 0) {
    events.push(thread_name(TID_FRAMES, "frames"));
    for (const f of frames) {
      const ts = us(f.t);
      events.push(event("frame", "frame", "C", ts, null, TID_FRAMES, frame_args(f)));
      // Also a duration bar for the scheduler pass, so a slow frame is
      // visible as width, not only as a counter spike.
      events.push(event("sched pass", "frame", "X", ts - us(f.sched_ms), us(f.sched_ms), TID_FRAMES));
    }
  }

  // ---- worst frames of the window ----
  // Already on the counter track above; repeated as labelled spans so a
  // stall can be found without scrubbing, at its real timestamp.
  if (worst.length > 0) {
    events.push(thread_name(TID_WORST, "worst frames"));
    for (const w of worst) {
      const f = w.sample;
      const args = frame_args(f);
      args.d_buffers = w.prev === null ? null : f.buffers - w.prev.buffers;
      args.d_pool_used_mb = w.prev === null ? null : f.pool.used_mb - w.prev.pool.used_mb;
      events.push(event(`stall ${f.dt_ms.toFixed(0)}ms`, "worst", "X", us(f.t - f.dt_ms), us(f.dt_ms), TID_WORST, args));
    }
  }

  // ---- long animation frames ----
  if (longs.length > 0) {
    events.push(thread_name(TID_LONG, "LoAF"));
    for (const l of longs) {
      const args = { blocking_ms: fixed(l.blocking_ms, 1), script_ms: fixed(l.script_ms, 1), render_ms: fixed(l.render_ms, 1) };
      events.push(event(`long frame ${l.duration_ms.toFixed(0)}ms`, "loaf", "X", us(l.t), us(l.duration_ms), TID_LONG, args));
    }
  }

  // Chrome's trace format keeps a top-level `metadata` object; a trace read
  // back later names the build it came from.
  return JSON.stringify({ traceEvents: events, displayTimeUnit: "ms", metadata: { build_number: BUILD_NUMBER, build_id: BUILD_ID, build_time: BUILD_TIME } });
}

export function ledger_trace_download(): void {
  const json = ledger_trace_json();
  const mb = (json.length / 1e6).toFixed(1);
  const name = `pxl-ledger-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  try {
    const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
    console.log(`[ledger] trace saved as ${name} — ${mb}MB. Open it at ui.perfetto.dev`);
  } catch (err) {
    console.log(`[ledger] trace built (${mb}MB) but the download failed:`, err);
  }
}
