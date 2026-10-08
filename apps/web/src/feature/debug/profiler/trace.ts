// @pxl/web/feature/debug/profiler/trace.ts

/**
 * The profiler window as Chrome's Trace Event Format, for ui.perfetto.dev.
 * Built here so the tens of megabytes are never the kernel heap's, and saved
 * as a file, since the clipboard handles that size badly.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/config/deploy.ts";
import { saveFile } from "@platform/save.ts";

import { type Frame, isList, LANES, LIST_NAMES, offthreadMs } from "./lane.ts";
import { failText } from "./report.ts";
import { profilerRingsRead } from "./view.ts";
import { longFramesSince } from "./long-frame.ts";

const PID = 1;
const TID_FRAMES = 1;
const TID_LONG = 2;
const TID_WORST = 3;
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

const threadName = (tid: u32, name: string) => event("thread_name", "__metadata", "M", 0, null, tid, { name });

/** Shared by the counter track and the worst-frame spans, so they agree. */
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
  // Vsync wait, compositor, GPU, GC, the shell's glue.
  args.offthread_ms = fixed(offthreadMs(f), 2);
  return args;
}

function profilerTraceJson(): string {
  const { frames, tasks, worst } = profilerRingsRead();
  const longs = longFramesSince(-Infinity);
  const events: TraceEvent[] = [];

  // ---- task spans, one thread per category ----
  const catTid = new Map<string, u32>();
  for (const r of tasks) {
    let tid = catTid.get(r.category);
    if (tid === undefined) {
      tid = TID_TASK_BASE + catTid.size;
      catTid.set(r.category, tid);
      events.push(threadName(tid, `task: ${r.category}`));
    }
    const args: Args = {
      outcome: r.outcome,
      priority: r.priority,
      queuedMs: r.start_ms === null ? null : fixed(r.start_ms - r.enqueue_ms, 1),
      ranMs: r.start_ms === null ? null : fixed(r.end_ms - r.start_ms, 1),
    };
    if (r.fail !== 0) {
      args.fail = failText(r.fail, r.detail);
      if (r.detail !== 0) args.detail = r.detail;
      if (r.at !== null) args.at = r.at;
    }
    for (const [k, v] of r.phases) args[k] = v;
    events.push(event(`${r.category}:${r.subject}`, `task,${r.outcome}`, "X", us(r.enqueue_ms), us(Math.max(0, r.end_ms - r.enqueue_ms)), tid, args));
  }

  // ---- per-frame counters (Perfetto graphs these) ----
  if (frames.length > 0) {
    events.push(threadName(TID_FRAMES, "frames"));
    for (const f of frames) {
      const ts = us(f.t);
      events.push(event("frame", "frame", "C", ts, null, TID_FRAMES, frameArgs(f)));
      events.push(event("sched pass", "frame", "X", ts - us(f.sched_ms), us(f.sched_ms), TID_FRAMES));
    }
  }

  // ---- worst frames of the window ----
  // Repeated as spans so a stall is found without scrubbing.
  if (worst.length > 0) {
    events.push(threadName(TID_WORST, "worst frames"));
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
    events.push(threadName(TID_LONG, "LoAF"));
    for (const l of longs) {
      const args = { blockingMs: fixed(l.blockingMs, 1), scriptMs: fixed(l.scriptMs, 1), renderMs: fixed(l.renderMs, 1) };
      events.push(event(`long frame ${l.durationMs.toFixed(0)}ms`, "loaf", "X", us(l.t), us(l.durationMs), TID_LONG, args));
    }
  }

  return JSON.stringify({ traceEvents: events, displayTimeUnit: "ms", metadata: { build_number: BUILD_NUMBER, build_id: BUILD_ID, build_time: BUILD_TIME } });
}

export async function profilerTraceSave(): Promise<string> {
  const json = profilerTraceJson();
  const mb = `${(json.length / 1e6).toFixed(1)} MB`;
  const name = `pxl-profiler-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  try {
    await saveFile(name, json, "application/json");
    console.log(`[profiler] trace saved as ${name} · ${mb} — open it at ui.perfetto.dev`);
    return `trace saved · ${mb}`;
  } catch (err) {
    console.log(`[profiler] trace built (${mb}) but saving it failed:`, err);
    return `trace not saved · ${mb}`;
  }
}
