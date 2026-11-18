// @pxl/client/ui/feature/debug/perf/trace.ts

/**
 * Writes the profiler's window in Chrome's Trace Event Format, which
 * ui.perfetto.dev opens. The trace is built on the client, so that its tens
 * of megabytes never take room in the core's heap. It is saved as a file,
 * because the clipboard handles text of that size badly.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";

import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/build.ts";

import { type Frame, isList, COLUMNS, read } from "@/service/core/perf.ts";
import { BIT_NAMES, effectsMs, LIST_NAMES, stageMs } from "./column.ts";
import { failText } from "./report.ts";
import { longFramesSince } from "./long-frame.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import * as share from "@/service/platform/share.ts";

const PID = 1;
const TID_FRAMES = 1;
const TID_LONG = 2;
const TID_TASK_BASE = 10;

type ArgMap = Record<string, f64 | string | null>;
interface TraceEvent {
  name: string;
  cat: string;
  ph: string;
  ts: f64;
  dur?: f64;
  pid: u32;
  tid: u32;
  args?: ArgMap;
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
  args?: ArgMap,
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

function frameArgs(f: Frame): ArgMap {
  const args: ArgMap = {};
  for (const { name } of COLUMNS) {
    if (isList(name)) {
      const names = LIST_NAMES[name];
      f[name].forEach((v, i) => (args[`${name}_${names[i]}`] = fixedN(v, 2)));
      continue;
    }
    const bits = BIT_NAMES[name];
    if (bits === undefined) args[name] = fixedN(f[name], 2);
    else
      bits.forEach((bit, k) => (args[`${name}_${bit}`] = (f[name] >> k) & 1));
  }
  return args;
}

/** The profiler's window as a trace, whole, its metadata saying how many
 * frames and tasks the tables dropped under the read. */
export async function perfTrace(): Promise<object> {
  const [{ frames, lost: lostFrameCount }, { tasks, lost: lostTaskCount }] =
    await Promise.all([read("frames"), read("tasks")]);
  const longs = longFramesSince(-Infinity);
  const events: TraceEvent[] = [];

  // Perfetto draws spans that overlap on one thread as nested. So each stream
  // has rows, and a task goes on the first of its stream's rows that is free
  // when the task is enqueued.
  const rows = new Map<string, f64[]>();
  const rowTid = new Map<string, u32>();
  const byEnqueue = [...tasks].sort((a, b) => a.enqueueMs - b.enqueueMs);
  for (const r of byEnqueue) {
    const ends = rows.get(r.stream) ?? [];
    rows.set(r.stream, ends);
    let k = ends.findIndex((end) => end <= r.enqueueMs);
    if (k < 0) k = ends.push(r.endMs) - 1;
    else ends[k] = r.endMs;
    const row = `${r.stream} ${k + 1}`;
    let tid = rowTid.get(row);
    if (tid === undefined) {
      tid = TID_TASK_BASE + rowTid.size;
      rowTid.set(row, tid);
      events.push(threadName(tid, `task: ${row}`));
    }
    const args: ArgMap = {
      priority: r.priority,
      queued_ms: r.startMs === null ? null : fixedN(r.startMs - r.enqueueMs, 1),
      ran_ms: r.startMs === null ? null : fixedN(r.endMs - r.startMs, 1),
    };
    if (r.fail !== 0) {
      args.fail = failText(r.fail, r.detail);
      if (r.at !== null) args.at = r.at;
    }
    for (const [k, v] of r.phases) args[k] = v;
    const cat = `task,${r.outcome}`;
    events.push(
      event(
        `${r.stream}:${r.path}`,
        cat,
        "X",
        us(r.enqueueMs),
        us(Math.max(0, r.endMs - r.enqueueMs)),
        tid,
        args,
      ),
    );
    if (r.startMs !== null) {
      events.push(
        event(
          "run",
          cat,
          "X",
          us(r.startMs),
          us(Math.max(0, r.endMs - r.startMs)),
          tid,
        ),
      );
    }
  }

  if (frames.length > 0) {
    events.push(threadName(TID_FRAMES, "frames"));
    for (const f of frames) {
      const ts = us(f.t);
      events.push(
        event("frame", "frame", "C", ts, null, TID_FRAMES, frameArgs(f)),
      );
      // A frame's record is taken as its stage ends, at `t`. So the stage
      // ends at `t`, the pump ends where the stage starts, the tick where the
      // pump starts, and the effects where the tick starts; the ground's fill
      // runs in the entry after the frame's and has no start of its own in the
      // record, so it is drawn from where the stage ends.
      const span = (name: string, endMs: f64, ms: f64) =>
        events.push(
          event(name, "frame", "X", ts - us(endMs + ms), us(ms), TID_FRAMES),
        );
      const stage = stageMs(f);
      span("stage", 0, stage);
      span("pump", stage, f.pump_ms);
      span("tick", stage + f.pump_ms, f.tick_ms);
      span("effects", stage + f.pump_ms + f.tick_ms, effectsMs(f));
      events.push(event("fill", "frame", "X", ts, us(f.fill_ms), TID_FRAMES));
    }
  }

  if (longs.length > 0) {
    events.push(threadName(TID_LONG, "long frames"));
    for (const l of longs) {
      const args = {
        blocking_ms: fixedN(l.blockingMs, 1),
        script_ms: fixedN(l.scriptMs, 1),
        render_ms: fixedN(l.renderMs, 1),
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

  return {
    traceEvents: events,
    displayTimeUnit: "ms",
    metadata: {
      build_number: BUILD_NUMBER,
      build_id: BUILD_ID,
      build_time: BUILD_TIME,
      lost_frame_cnt: lostFrameCount,
      lost_task_cnt: lostTaskCount,
    },
  };
}

export async function perfTraceSave(): Promise<string> {
  const json = JSON.stringify(await perfTrace());
  const mb = `${fixed(json.length / 1e6, 1)} MB`;
  const name = `perf-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  try {
    await share.save(name, json, "application/json");
    console.log(
      `[perf] trace saved as ${name} · ${mb} — open it at ui.perfetto.dev`,
    );
    return `trace saved · ${mb}`;
  } catch (err) {
    console.log(`[perf] trace built (${mb}) but saving it failed:`, err);
    return `trace not saved · ${mb}`;
  }
}
