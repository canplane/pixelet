// @pxl/client/shell/debug/profiler/view.ts

/**
 * What the profiler's panel and report are made of: the kernel's records as
 * abi/profiler.ts decodes them, with the place's long frames beside them
 * (long-frame.ts) and the build they were taken on.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/deploy.ts";
import {
  type FrameSummary,
  profiler_counts,
  profiler_summary,
  profiler_tasks,
  profiler_worst,
  type Step,
  type TaskRecord,
  type TaskSummary,
  type Worst,
} from "@/abi/profiler.ts";
import type { LongFrame } from "@/bracket.tsx";
import { longFramesSince } from "./long-frame.ts";

const RECENT_TASKS = 20;

export interface ProfilerReport {
  t: f64;
  build: { number: string; id: string; time: string };
  frames: FrameSummary | null;
  steps: Step[];
  tasks: TaskSummary[];
  recent: TaskRecord[];
  worst: Worst[];
  longs: LongFrame[];
}

/** Frames a second over a window: its intervals over its span, none under two frames. */
export const fps = (f: { n: f64; seconds: f64 }): f64 | null =>
  f.n >= 2 && f.seconds > 0 ? (f.n - 1) / f.seconds : null;

/**
 * Frames a second while frames are drawn: the moving frames over their
 * intervals summed, none where none moved. A window's average (`fps`)
 * counts the still spans between moves as frames not drawn, and the median
 * interval holds at the refresh until half the frames miss it.
 */
export const movingFps = (f: { moving: f64; moving_ms: f64 }): f64 | null =>
  f.moving >= 1 && f.moving_ms > 0 ? (1000 * f.moving) / f.moving_ms : null;

/** The window the header's rate is over: short, so a dip shows as it happens. */
export const RECENT_MS = 1_000;

export interface Gauge {
  /** The frames and tasks the rings hold, as RINGS counts them. */
  rec: {
    recording: boolean;
    elapsed_ms: f64;
    tasks: u32;
    frames: u32;
    truncated: boolean;
  };
  frames: FrameSummary | null;
  tasks: TaskSummary[];
  view: Record<string, f64>;
}

export function profilerGaugeRead(windowMs: f64): Gauge {
  const s = profiler_summary(windowMs);
  const held = profiler_counts();
  return {
    rec: {
      recording: s.rec.recording,
      elapsed_ms: s.rec.elapsed_ms,
      tasks: held.tasks,
      frames: held.frames,
      truncated: s.rec.truncated,
    },
    frames: s.frames,
    tasks: s.tasks,
    view: s.view,
  };
}

export function profilerReportRead(): ProfilerReport {
  const s = profiler_summary(null);
  return {
    t: performance.now(),
    build: { number: BUILD_NUMBER, id: BUILD_ID, time: BUILD_TIME },
    frames: s.frames,
    steps: s.steps,
    tasks: s.tasks,
    recent: profiler_tasks(RECENT_TASKS),
    worst: profiler_worst(),
    longs: longFramesSince(-Infinity),
  };
}
