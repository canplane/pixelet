// @pxl/client/ui/debug/perf/view.ts

/**
 * What the profiler's panel and report are made of: the core's records as
 * abi/perf.ts decodes them, with the place's long frames beside them
 * (long-frame.ts) and the build they were taken on.
 */

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/deploy.ts";
import {
  type FrameSummary,
  read,
  type RecordingSection,
  type Step,
  type TaskRecord,
  type TaskSummary,
  type ViewSection,
  type Worst,
} from "@/port/perf.ts";
import type { LongFrame } from "@/service/types.ts";
import { longFramesSince } from "./long-frame.ts";

const RECENT_TASKS = 20;

export interface PerfReport {
  t: f64;
  build: { number: string; id: string; time: string };
  frames: FrameSummary | null;
  steps: Step[];
  tasks: TaskSummary[];
  recent: TaskRecord[];
  worst: Worst[];
  longs: LongFrame[];
  /** What the frames held at the most or the least since the recording
   * started, and where the view is as it is read. */
  recording: RecordingSection;
  view: ViewSection;
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
  /** The frames over the last `RECENT_MS`, the header's rate. */
  recent: FrameSummary | null;
  tasks: TaskSummary[];
  view: ViewSection;
}

export async function perfGaugeRead(windowMs: f64): Promise<Gauge> {
  const [s, held, recent] = await Promise.all([
    read("summary", windowMs),
    read("counts"),
    read("summary", RECENT_MS),
  ]);
  return {
    rec: {
      recording: s.recording_state.recording,
      elapsed_ms: s.recording_state.elapsed_ms,
      tasks: held.tasks,
      frames: held.frames,
      truncated: s.recording_state.truncated,
    },
    frames: s.frames,
    recent: recent.frames,
    tasks: s.tasks,
    view: s.view,
  };
}

export async function perfReportRead(): Promise<PerfReport> {
  const [s, recent, worst] = await Promise.all([
    read("summary", null),
    read("tasks", RECENT_TASKS),
    read("worst"),
  ]);
  return {
    t: performance.now(),
    build: { number: BUILD_NUMBER, id: BUILD_ID, time: BUILD_TIME },
    frames: s.frames,
    steps: s.steps,
    tasks: s.tasks,
    recent,
    worst,
    longs: longFramesSince(-Infinity),
    recording: s.recording,
    view: s.view,
  };
}
