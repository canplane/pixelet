// @pxl/web/module/world/sched/trace.ts

/**
 * Chrome Trace Event Format dump of the ledger rings, openable in Perfetto
 * (ui.perfetto.dev) or chrome://tracing. Aggregates the same numbers the dev
 * overlay shows, but on one timeline — the "trace, not just aggregates"
 * point in issue/20260909-cooperative-scheduler-and-task-ledger.md:
 * p95 says a category is slow, the timeline shows it's slow *because* it's
 * contending with a fetch burst that the same pan kicked off.
 *
 * Dev instrument. Recording covers the whole window (ledger.ts), so a trace
 * is routinely tens of MB — saved as a file, see ledger_trace_download.
 */

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/lib/env/config.ts";
import { type FrameSample, ledger_frames_snapshot, ledger_long_frames_snapshot, ledger_snapshot, ledger_worst_frames } from "./ledger.ts";

type TraceEvent = {
  name: string;
  cat: string;
  ph: "X" | "C" | "M" | "i";
  ts: number;
  dur?: number;
  pid: number;
  tid: number;
  args?: Record<string, unknown>;
  s?: "g";
};

const PID = 1;
const TID_FRAMES = 1;
const TID_LONG = 2;
const TID_WORST = 3;
// task threads are assigned per category from here up
const TID_TASK_BASE = 10;

const ms_to_us = (ms: number): number => Math.round(ms * 1000);

function thread_name(tid: number, name: string): TraceEvent {
  return { name: "thread_name", cat: "__metadata", ph: "M", ts: 0, pid: PID, tid, args: { name } };
}

/** One frame's numbers as trace args — the counter track and the worst-frame
 * spans both carry them, so the two never disagree on what a frame was. */
function frame_args(f: FrameSample): Record<string, unknown> {
  return {
    dt_ms: +f.dt_ms.toFixed(2),
    task_ms: +f.task_ms.toFixed(2),
    // everything the main thread's own task did not account for:
    // vsync wait, compositor, GPU, GC between frames.
    offthread_ms: +Math.max(0, f.dt_ms - f.task_ms).toFixed(2),
    gpu_ms: +f.gpu_ms.toFixed(2),
    sched_ms: +f.sched_ms.toFixed(2),
    budget_ms: +f.budget_ms.toFixed(2),
    frame_ms: +f.frame_ms.toFixed(2),
    settled: f.settled ? 1 : 0,
    ...Object.fromEntries(Object.entries(f.phases).map(([k, v]) => [`phase_${k}_ms`, +v.toFixed(2)])),
    recheck_ms: +f.cpu.recheck_ms.toFixed(2),
    commit_ms: +f.cpu.commit_ms.toFixed(2),
    triangles: f.triangles,
    ...Object.fromEntries(Object.entries(f.census).map(([tag, [, tris]]) => [`tris_${tag}`, tris])),
    geometries: f.geometries,
    textures: f.textures,
    mounted: f.mounts.added,
    unmounted: f.mounts.removed,
    rerendered: f.mounts.rendered,
    heap_mb: f.heap_mb,
    pool_live_mb: f.pool.live_mb,
    pool_idle_mb: f.pool.idle_mb,
    pool_hits: f.pool.hits,
    pool_misses: f.pool.misses,
    scans: f.work.scans,
    bakes: f.work.bakes,
    runs: f.work.runs,
    tile_pages: f.tile_pages,
    poi_count: f.poi_count,
    lod_pending: f.lod.pending,
    lod_in_frustum: f.lod.in_frustum,
    lod_offscreen_pending: f.lod.offscreen_pending,
    lod_oldest_release_ms: Math.round(f.lod.oldest_release_ms),
    lod_oldest_visible_ms: Math.round(f.lod.oldest_visible_ms),
    draw_calls: f.draw_calls,
    fill_mpx: +((f.px * f.fx_passes) / 1e6).toFixed(2),
    q_terrain: f.q_terrain,
    q_poi: f.q_poi,
  };
}

/** The trace as a JSON string. */
export function ledger_trace_json(): string {
  const events: TraceEvent[] = [];
  const tasks = ledger_snapshot();
  const frames = ledger_frames_snapshot();
  const longs = ledger_long_frames_snapshot();

  // ---- task spans, one thread per category ----
  const cat_tid = new Map<string, number>();
  for (const rec of tasks) {
    let tid = cat_tid.get(rec.category);
    if (tid === undefined) {
      tid = TID_TASK_BASE + cat_tid.size;
      cat_tid.set(rec.category, tid);
      events.push(thread_name(tid, `task: ${rec.category}`));
    }
    const end = rec.end_ms;
    events.push({
      name: rec.key,
      cat: `task,${rec.outcome}`,
      ph: "X",
      ts: ms_to_us(rec.enqueue_ms),
      dur: ms_to_us(Math.max(0, end - rec.enqueue_ms)),
      pid: PID,
      tid,
      args: {
        outcome: rec.outcome,
        priority: rec.priority,
        queued_ms: rec.start_ms !== null ? +(rec.start_ms - rec.enqueue_ms).toFixed(1) : null,
        ran_ms: rec.start_ms !== null ? +(end - rec.start_ms).toFixed(1) : null,
        transport_ms: rec.transport_ms,
        error: rec.error,
        ...rec.phases,
      },
    });
  }

  // ---- per-frame counters (Perfetto graphs these) ----
  if (frames.length > 0) {
    events.push(thread_name(TID_FRAMES, "frames"));
    for (const f of frames) {
      const ts = ms_to_us(f.t);
      events.push({
        name: "frame",
        cat: "frame",
        ph: "C",
        ts,
        pid: PID,
        tid: TID_FRAMES,
        args: frame_args(f),
      });
      // also a duration bar for the scheduler pass, so a slow frame is
      // visible as width, not only as a counter spike
      events.push({
        name: "sched pass",
        cat: "frame",
        ph: "X",
        ts: ts - ms_to_us(f.sched_ms),
        dur: ms_to_us(f.sched_ms),
        pid: PID,
        tid: TID_FRAMES,
      });
    }
  }

  // ---- worst frames of the window ----
  // Already on the counter track above; repeated as labelled spans so a
  // stall can be found without scrubbing, at its real timestamp so it lines
  // up with whatever else the trace holds.
  const worst = ledger_worst_frames();
  if (worst.length > 0) {
    events.push(thread_name(TID_WORST, "worst frames"));
    for (const { sample: f, prev } of worst) {
      events.push({
        name: `stall ${f.dt_ms.toFixed(0)}ms`,
        cat: "worst",
        ph: "X",
        ts: ms_to_us(f.t - f.dt_ms),
        dur: ms_to_us(f.dt_ms),
        pid: PID,
        tid: TID_WORST,
        args: {
          ...frame_args(f),
          d_geometries: prev ? f.geometries - prev.geometries : null,
          d_heap_mb: prev ? f.heap_mb - prev.heap_mb : null,
        },
      });
    }
  }

  // ---- long animation frames ----
  if (longs.length > 0) {
    events.push(thread_name(TID_LONG, "LoAF"));
    for (const l of longs) {
      events.push({
        name: `long frame ${l.duration_ms.toFixed(0)}ms`,
        cat: "loaf",
        ph: "X",
        ts: ms_to_us(l.t),
        dur: ms_to_us(l.duration_ms),
        pid: PID,
        tid: TID_LONG,
        args: {
          blocking_ms: +l.blocking_ms.toFixed(1),
          script_ms: +l.script_ms.toFixed(1),
          render_ms: +l.render_ms.toFixed(1),
        },
      });
    }
  }

  // Chrome's trace format keeps a top-level `metadata` object; a trace read
  // back later names the build it came from.
  return JSON.stringify({ traceEvents: events, displayTimeUnit: "ms", metadata: { build_number: BUILD_NUMBER, build_id: BUILD_ID, build_time: BUILD_TIME } });
}

/**
 * Saves the trace as a file and says so.
 *
 * A download rather than the clipboard: recording is exhaustive for the whole
 * window now, so a trace is routinely tens of MB, which the clipboard handles
 * badly or not at all — and Perfetto wants a file anyway. Wired to `window` in
 * dev (see ledger_overlay.tsx) so it is reachable from the console too, where
 * it also returns the JSON.
 */
export function ledger_trace_download(): string {
  const json = ledger_trace_json();
  const events = JSON.parse(json).traceEvents.length;
  const name = `pxl-ledger-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  try {
    const url = URL.createObjectURL(new Blob([json], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
    console.log(`[ledger] trace saved as ${name} — ${events} events, ${(json.length / 1e6).toFixed(1)}MB. Open it at ui.perfetto.dev`);
  } catch (err) {
    console.log(`[ledger] trace built (${events} events) but the download failed:`, err);
  }
  return json;
}
