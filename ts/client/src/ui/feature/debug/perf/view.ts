// @pxl/client/ui/feature/debug/perf/view.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { BUILD_ID, BUILD_NUMBER, BUILD_TIME } from "@/build.ts";
import {
  type FrameSummary,
  read,
  type RecordingSection,
  type RecordingState,
  start,
  type Step,
  type TaskRecord,
  type TaskSummary,
  type ViewSection,
  type Worst,
} from "@/service/core/perf.ts";
import { hover } from "@/service/core/input.ts";
import type { LongFrame } from "@/service/platform/types.ts";
import { longFramesClear, longFramesSince } from "./long-frame.ts";

const RECENT_TASKS = 20;

/** Starts the profiler's window afresh: the core's tables and the platform's
 * long frames together, so both cover the same span. */
export function perfWindowStart(): void {
  start();
  longFramesClear();
}

export interface PerfReport {
  t: f64;
  build: { number: string; id: string; time: string };
  frames: FrameSummary | null;
  steps: Step[];
  tasks: TaskSummary[];
  recent: TaskRecord[];
  worst: Worst[];
  longs: LongFrame[];
  /** Whether the window lost records it had no room for. */
  truncated: boolean;
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
export const movingFps = (f: { moving: f64; movingMs: f64 }): f64 | null =>
  f.moving >= 1 && f.movingMs > 0 ? (1000 * f.moving) / f.movingMs : null;

/** The span the panel title's frame rate is taken over. It is short so that
 * a dip shows as it happens. */
const RECENT_MS = 1_000;

export interface Gauge {
  recordingState: RecordingState;
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
    recordingState: s.recordingState,
    frames: s.frames,
    recent: recent.frames,
    tasks: s.tasks,
    view: s.view,
  };
}

const AT_POLL_MS = 50;
const AT_WAIT_MS = 2_000;

/**
 * The view once it holds what the drawn pixel under (`x`, `y`) draws, a
 * point in the view's logical pixels. The pointer is hovered there, and the
 * view is read, each read asking the pixel's words back, until it holds those
 * of a frame no older than the last one drawn when it was hovered (`NOW.view`
 * `hover_frame`). Null where none land within `AT_WAIT_MS`, as where no world
 * is drawn at that pixel.
 */
export async function perfViewReadAt(
  x: f64,
  y: f64,
): Promise<ViewSection | null> {
  hover(x, y);
  const draw = await read("draw");
  for (let waited = 0; waited < AT_WAIT_MS; waited += AT_POLL_MS) {
    await new Promise((landed) => setTimeout(landed, AT_POLL_MS));
    const view = await read("view");
    if (view.hover_frame >= draw.frame_cnt) return view;
  }
  return null;
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
    recent: recent.tasks,
    worst,
    longs: longFramesSince(-Infinity),
    truncated: s.recordingState.truncated,
    recording: s.recording,
    view: s.view,
  };
}
