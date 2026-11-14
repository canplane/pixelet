// @pxl/client/ui/feature/debug/perf/view.ts

import type { f64, u32 } from "@xpute/kit/abi/word.ts";
import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/build.ts";
import {
  type FrameSummary,
  read,
  type RecordingSection,
  type Step,
  type TaskRecord,
  type TaskSummary,
  type ViewSection,
  type Worst,
} from "@/service/core/perf.ts";
import type { LongFrame } from "@/service/platform/types.ts";
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
  recording: RecordingSection;
  view: ViewSection;
}

/** The frame rate over a window. Its `n` frames bound `n - 1` intervals,
 * which are divided by the window's span. Fewer than two frames give no
 * rate. */
export const fps = (f: { n: f64; seconds: f64 }): f64 | null =>
  f.n >= 2 && f.seconds > 0 ? (f.n - 1) / f.seconds : null;

/**
 * The frame rate while frames are being drawn: the frames that moved, over
 * the sum of their intervals. When no frame moved there is no rate. The
 * window's average (`fps`) would count the still spans between moves as
 * frames not drawn, and the median interval stays at the display's refresh
 * until half the frames miss it, so neither shows this rate.
 */
export const movingFps = (f: { moving: f64; moving_ms: f64 }): f64 | null =>
  f.moving >= 1 && f.moving_ms > 0 ? (1000 * f.moving) / f.moving_ms : null;

/** The span the panel title's frame rate is taken over. It is short so that
 * a dip shows as it happens. */
export const RECENT_MS = 1_000;

export interface Gauge {
  rec: {
    recording: boolean;
    elapsed_ms: f64;
    task_cnt: u32;
    frame_cnt: u32;
    truncated: boolean;
  };
  frames: FrameSummary | null;
  recent: FrameSummary | null;
  tasks: TaskSummary[];
  view: ViewSection;
}

export async function perfGaugeRead(windowMs: f64): Promise<Gauge> {
  const [s, recent] = await Promise.all([
    read("summary", windowMs),
    read("summary", RECENT_MS),
  ]);
  return {
    rec: {
      recording: s.recording_state.recording,
      elapsed_ms: s.recording_state.elapsed_ms,
      task_cnt: s.recording_state.task_cnt,
      frame_cnt: s.recording_state.frame_cnt,
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
