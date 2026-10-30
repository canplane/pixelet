// @pxl/client/shell/debug/profiler/trace.ts

/**
 * The profiler window as Chrome's Trace Event Format, for ui.perfetto.dev.
 * Built here so the tens of megabytes are never the kernel heap's, and saved
 * as a file, since the clipboard handles that size badly.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/deploy.ts";

import {
  type Frame,
  is_list,
  LANES,
  profiler_frames,
  profiler_tasks,
} from "@/port/profiler.ts";
import { LIST_NAMES } from "./lane.ts";
import { failText } from "./report.ts";
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
const fixedN = (n: number, d: number) => +fixed(n, d);

function event(
  name: string,
  cat: string,
  ph: string,
  ts: f64,
  dur: f64 | null,
  tid: u32,
  args?: Args,
): TraceEvent {
  return {
    name,
    cat,
    ph,
    ts,
    ...(dur === null ? {} : { dur }),
    pid: PID,
    tid,
    ...(args ? { args } : {}),
  };
}

const threadName = (tid: u32, name: string) =>
  event("thread_name", "__metadata", "M", 0, null, tid, { name });

/** A frame's lanes as the counter track's arguments. */
function frameArgs(f: Frame): Args {
  const args: Args = {};
  for (const { name } of LANES) {
    if (!is_list(name)) {
      args[name] = fixedN(f[name], 2);
      continue;
    }
    const names = LIST_NAMES[name];
    if (name === "census") {
      f.census.forEach(([d, t], i) => {
        args[`census_${names[i]}_draws`] = d;
        args[`census_${names[i]}_tris`] = t;
      });
    } else {
      f[name].forEach((v, i) => (args[`${name}_${names[i]}`] = fixedN(v, 2)));
    }
  }
  // Vsync wait, compositor, GPU, GC, the shell's glue.
  return args;
}

function profilerTraceJson(): string {
  const frames = profiler_frames();
  const tasks = profiler_tasks();
  const longs = longFramesSince(-Infinity);
  const events: TraceEvent[] = [];

  // ---- task spans: a stream's rows, each task on the first that is free
  // by its enqueue, so tasks that overlap never nest falsely ----
  const rows = new Map<string, f64[]>();
  const rowTid = new Map<string, u32>();
  const byEnqueue = [...tasks].sort((a, b) => a.enqueue_ms - b.enqueue_ms);
  for (const r of byEnqueue) {
    const ends = rows.get(r.stream) ?? [];
    rows.set(r.stream, ends);
    let k = ends.findIndex((end) => end <= r.enqueue_ms);
    if (k < 0) k = ends.push(r.end_ms) - 1;
    else ends[k] = r.end_ms;
    const row = `${r.stream} ${k + 1}`;
    let tid = rowTid.get(row);
    if (tid === undefined) {
      tid = TID_TASK_BASE + rowTid.size;
      rowTid.set(row, tid);
      events.push(threadName(tid, `task: ${row}`));
    }
    // The outcome is the event's category, and the detail is in `fail`'s text.
    const args: Args = {
      priority: r.priority,
      queuedMs:
        r.start_ms === null ? null : fixedN(r.start_ms - r.enqueue_ms, 1),
      ranMs: r.start_ms === null ? null : fixedN(r.end_ms - r.start_ms, 1),
    };
    if (r.fail !== 0) {
      args.fail = failText(r.fail, r.detail);
      if (r.at !== null) args.at = r.at;
    }
    for (const [k, v] of r.phases) args[k] = v;
    const cat = `task,${r.outcome}`;
    events.push(
      event(
        `${r.stream}:${r.subject}`,
        cat,
        "X",
        us(r.enqueue_ms),
        us(Math.max(0, r.end_ms - r.enqueue_ms)),
        tid,
        args,
      ),
    );
    // Its run, from its start, inside it.
    if (r.start_ms !== null) {
      events.push(
        event(
          "run",
          cat,
          "X",
          us(r.start_ms),
          us(Math.max(0, r.end_ms - r.start_ms)),
          tid,
        ),
      );
    }
  }

  // ---- per-frame counters (Perfetto graphs these) ----
  if (frames.length > 0) {
    events.push(threadName(TID_FRAMES, "frames"));
    for (const f of frames) {
      const ts = us(f.t);
      events.push(
        event("frame", "frame", "C", ts, null, TID_FRAMES, frameArgs(f)),
      );
      // The record is taken as the stage ends: the stage, the tick before
      // it and the effects before that.
      const span = (name: string, end_ms: f64, ms: f64) =>
        events.push(
          event(name, "frame", "X", ts - us(end_ms + ms), us(ms), TID_FRAMES),
        );
      span("stage", 0, f.stage_ms);
      span("tick", f.stage_ms, f.tick_ms);
      span("effects", f.stage_ms + f.tick_ms, f.effects_ms);
    }
  }

  // ---- long animation frames ----
  if (longs.length > 0) {
    events.push(threadName(TID_LONG, "long frames"));
    for (const l of longs) {
      const args = {
        blockingMs: fixedN(l.blockingMs, 1),
        scriptMs: fixedN(l.scriptMs, 1),
        renderMs: fixedN(l.renderMs, 1),
      };
      events.push(
        event(
          `${l.source} ${fixed(l.durationMs, 0)}ms`,
          l.source,
          "X",
          us(l.t),
          us(l.durationMs),
          TID_LONG,
          args,
        ),
      );
    }
  }

  return JSON.stringify({
    traceEvents: events,
    displayTimeUnit: "ms",
    metadata: {
      build_number: BUILD_NUMBER,
      build_id: BUILD_ID,
      build_time: BUILD_TIME,
    },
  });
}

export async function profilerTraceSave(): Promise<string> {
  const json = profilerTraceJson();
  const mb = `${fixed(json.length / 1e6, 1)} MB`;
  const name = `profiler-${new Date()
    .toISOString()
    .replace(/[:.]/g, "-")}.json`;
  try {
    await bracket.share.save(name, json, "application/json");
    console.log(
      `[profiler] trace saved as ${name} · ${mb} — open it at ui.perfetto.dev`,
    );
    return `trace saved · ${mb}`;
  } catch (err) {
    console.log(`[profiler] trace built (${mb}) but saving it failed:`, err);
    return `trace not saved · ${mb}`;
  }
}
