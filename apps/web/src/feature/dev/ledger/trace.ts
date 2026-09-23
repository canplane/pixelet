// @pxl/web/feature/dev/ledger/trace.ts

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

import type { f64, u32 } from "@xpute/core/abi/word.ts";
import { round } from "@xpute/core/math/scalar.ts";

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/config/deploy.ts";

import { type Frame, isList, LANES, LIST_NAMES } from "./lane.ts";
import { failText } from "./report.ts";
import { ledgerRingsRead } from "./view.ts";
import { longFramesSince } from "./long-frame.ts";

const PID = 1;
const TID_FRAMES = 1;
const TID_LONG = 2;
const TID_WORST = 3;
/** Task threads are assigned per category from here up. */
const TID_TASK_BASE = 10;

type Args = Record<string, f64 | string | null>;
interface TraceEvent {
  name: string;
  cat: string;
  ph: string;
  ts: f64;
  dur?: f64;
  pid: u32;
  tid: u32;
  args?: Args;
}

const us = (ms: f64) => round(ms * 1000);
const fixed = (n: number, d: number) => +n.toFixed(d);

function event(name: string, cat: string, ph: string, ts: f64, dur: f64 | null, tid: u32, args?: Args): TraceEvent {
  return { name, cat, ph, ts, ...(dur === null ? {} : { dur }), pid: PID, tid, ...(args ? { args } : {}) };
}

const thread_name = (tid: u32, name: string) => event("thread_name", "__metadata", "M", 0, null, tid, { name });

/** One frame's numbers as trace args — the counter track and the worst-frame
 * spans both carry them, so the two never disagree on what a frame was. Every
 * lane, by its own name, and a list lane's words by the lane's and theirs:
 * what the kernel adds is on the timeline without a line written here. */
function frameArgs(f: Frame): Args {
  const args: Args = {};
  for (const { name } of LANES) {
    if (!isList(name)) {
      args[name] = fixed(f[name], 2);
      continue;
    }
    const names = LIST_NAMES[name];
    if (name === "census") f.census.forEach(([, t], i) => (args[`census_${names[i]}_tris`] = t));
    else f[name].forEach((v, i) => (args[`${name}_${names[i]}`] = fixed(v, 2)));
  }
  // Everything the kernel's turn did not account for: vsync wait,
  // compositor, GPU, GC between frames, the shell's glue.
  args.offthread_ms = fixed(Math.max(0, f.dt_ms - f.task_ms), 2);
  return args;
}

/** The trace as a JSON string. */
export function ledgerTraceJson(): string {
  const { frames, tasks, worst } = ledgerRingsRead();
  const longs = longFramesSince(-Infinity);
  const events: TraceEvent[] = [];

  // ---- task spans, one thread per category ----
  const catTid = new Map<string, u32>();
  for (const r of tasks) {
    let tid = catTid.get(r.category);
    if (tid === undefined) {
      tid = TID_TASK_BASE + catTid.size;
      catTid.set(r.category, tid);
      events.push(thread_name(tid, `task: ${r.category}`));
    }
    const args: Args = {
      outcome: r.outcome,
      priority: r.priority,
      queuedMs: r.start_ms === null ? null : fixed(r.start_ms - r.enqueue_ms, 1),
      ranMs: r.start_ms === null ? null : fixed(r.end_ms - r.start_ms, 1),
      transport_ms: r.transport_ms,
    };
    if (r.fail !== 0) {
      args.fail = failText(r.fail, r.detail);
      if (r.detail !== 0) args.detail = r.detail;
    }
    for (const [k, v] of r.phases) args[k] = v;
    events.push(event(`${r.category}:${r.subject}`, `task,${r.outcome}`, "X", us(r.enqueue_ms), us(Math.max(0, r.end_ms - r.enqueue_ms)), tid, args));
  }

  // ---- per-frame counters (Perfetto graphs these) ----
  if (frames.length > 0) {
    events.push(thread_name(TID_FRAMES, "frames"));
    for (const f of frames) {
      const ts = us(f.t);
      events.push(event("frame", "frame", "C", ts, null, TID_FRAMES, frameArgs(f)));
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
      const args = frameArgs(f);
      args.d_buffers = w.prev === null ? null : f.buffers - w.prev.buffers;
      args.d_heap_used_mb = w.prev === null ? null : f.heap_used_mb - w.prev.heap_used_mb;
      events.push(event(`stall ${f.dt_ms.toFixed(0)}ms`, "worst", "X", us(f.t - f.dt_ms), us(f.dt_ms), TID_WORST, args));
    }
  }

  // ---- long animation frames ----
  if (longs.length > 0) {
    events.push(thread_name(TID_LONG, "LoAF"));
    for (const l of longs) {
      const args = { blockingMs: fixed(l.blockingMs, 1), scriptMs: fixed(l.scriptMs, 1), renderMs: fixed(l.renderMs, 1) };
      events.push(event(`long frame ${l.durationMs.toFixed(0)}ms`, "loaf", "X", us(l.t), us(l.durationMs), TID_LONG, args));
    }
  }

  // Chrome's trace format keeps a top-level `metadata` object; a trace read
  // back later names the build it came from.
  return JSON.stringify({ traceEvents: events, displayTimeUnit: "ms", metadata: { build_number: BUILD_NUMBER, build_id: BUILD_ID, build_time: BUILD_TIME } });
}

export function ledgerTraceDownload(): void {
  const json = ledgerTraceJson();
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
