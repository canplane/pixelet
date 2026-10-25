// @pxl/client/shell/debug/profiler/trace.ts

/**
 * The profiler window as Chrome's Trace Event Format, for ui.perfetto.dev.
 * Built here so the tens of megabytes are never the kernel heap's, and saved
 * as a file, since the clipboard handles that size badly.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/config/deploy.ts";

import { type Frame, isList, LANES, LIST_NAMES } from "./lane.ts";
import { failText } from "./report.ts";
import { profilerRingsRead } from "./view.ts";
import { longFramesSince } from "./long-frame.ts";
import { fixed } from "@/shell/fixed.ts";
import { bracket } from "@/bracket.tsx";

const PID = 1;
const TID_FRAMES = 1;
const TID_LONG = 2;
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
const fixed_n = (n: number, d: number) => +fixed(n, d);

function event(name: string, cat: string, ph: string, ts: f64, dur: f64 | null, tid: u32, args?: Args): TraceEvent {
  return { name, cat, ph, ts, ...(dur === null ? {} : { dur }), pid: PID, tid, ...(args ? { args } : {}) };
}

const threadName = (tid: u32, name: string) => event("thread_name", "__metadata", "M", 0, null, tid, { name });

/** A frame's lanes as the counter track's arguments. */
function frameArgs(f: Frame): Args {
  const args: Args = {};
  for (const { name } of LANES) {
    if (!isList(name)) {
      args[name] = fixed_n(f[name], 2);
      continue;
    }
    const names = LIST_NAMES[name];
    if (name === "census") {
      f.census.forEach(([d, t], i) => {
        args[`census_${names[i]}_draws`] = d;
        args[`census_${names[i]}_tris`] = t;
      });
    } else f[name].forEach((v, i) => (args[`${name}_${names[i]}`] = fixed_n(v, 2)));
  }
  // Vsync wait, compositor, GPU, GC, the shell's glue.
  return args;
}

function profilerTraceJson(): string {
  const { frames, tasks } = profilerRingsRead();
  const longs = longFramesSince(-Infinity);
  const events: TraceEvent[] = [];

  // ---- task spans, one thread per stream ----
  const catTid = new Map<string, u32>();
  for (const r of tasks) {
    let tid = catTid.get(r.stream);
    if (tid === undefined) {
      tid = TID_TASK_BASE + catTid.size;
      catTid.set(r.stream, tid);
      events.push(threadName(tid, `task: ${r.stream}`));
    }
    // The outcome is the event's category, and the detail is in `fail`'s text.
    const args: Args = {
      priority: r.priority,
      queuedMs: r.start_ms === null ? null : fixed_n(r.start_ms - r.enqueue_ms, 1),
      ranMs: r.start_ms === null ? null : fixed_n(r.end_ms - r.start_ms, 1),
    };
    if (r.fail !== 0) {
      args.fail = failText(r.fail, r.detail);
      if (r.at !== null) args.at = r.at;
    }
    for (const [k, v] of r.phases) args[k] = v;
    events.push(event(`${r.stream}:${r.subject}`, `task,${r.outcome}`, "X", us(r.enqueue_ms), us(Math.max(0, r.end_ms - r.enqueue_ms)), tid, args));
  }

  // ---- per-frame counters (Perfetto graphs these) ----
  if (frames.length > 0) {
    events.push(threadName(TID_FRAMES, "frames"));
    for (const f of frames) {
      const ts = us(f.t);
      events.push(event("frame", "frame", "C", ts, null, TID_FRAMES, frameArgs(f)));
      // The record is taken as the stage ends, and the tick ran just before it.
      events.push(event("tick", "frame", "X", ts - us(f.stage_ms + f.tick_ms), us(f.tick_ms), TID_FRAMES));
    }
  }

  // ---- long animation frames ----
  if (longs.length > 0) {
    events.push(threadName(TID_LONG, "long frames"));
    for (const l of longs) {
      const args = { blockingMs: fixed_n(l.blockingMs, 1), scriptMs: fixed_n(l.scriptMs, 1), renderMs: fixed_n(l.renderMs, 1) };
      events.push(event(`${l.source} ${fixed(l.durationMs, 0)}ms`, l.source, "X", us(l.t), us(l.durationMs), TID_LONG, args));
    }
  }

  return JSON.stringify({ traceEvents: events, displayTimeUnit: "ms", metadata: { build_number: BUILD_NUMBER, build_id: BUILD_ID, build_time: BUILD_TIME } });
}

export async function profilerTraceSave(): Promise<string> {
  const json = profilerTraceJson();
  const mb = `${fixed(json.length / 1e6, 1)} MB`;
  const name = `pxl-profiler-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  try {
    await bracket.share.save(name, json, "application/json");
    console.log(`[profiler] trace saved as ${name} · ${mb} — open it at ui.perfetto.dev`);
    return `trace saved · ${mb}`;
  } catch (err) {
    console.log(`[profiler] trace built (${mb}) but saving it failed:`, err);
    return `trace not saved · ${mb}`;
  }
}
