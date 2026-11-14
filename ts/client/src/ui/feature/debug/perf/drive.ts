// @pxl/client/ui/feature/debug/perf/drive.ts

/**
 * The one standard measurement (`pxl.perf.drive`). It puts the renderer
 * through the worst case it meets, the same way every time. The display's
 * pixels are fixed, the shadows and the profiler are on, the sky is at 07:00,
 * and the camera passes through the ground, so that the eye takes the same
 * path wherever it stands. Through the core's own input, the camera is
 * zoomed to its farthest
 * and tilted to just over the horizon. Then, for the whole window, the view
 * turns a full circle every `TURN_S` seconds while the wheel sweeps the zoom
 * in and out, both paced by the clock. The result is the window's report.
 * Two runs of one build give two matching windows, so a change shows as the
 * difference between their reports.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  type PointerPhase,
  POINTER_PHASE_DOWN,
  POINTER_PHASE_MOVE,
  POINTER_PHASE_UP,
  POINTER_TYPE_MOUSE,
} from "@/abi/input.enum.ts";
import { SETTING } from "@/abi/schema.spec.ts";
import { perf_report_text } from "./report.ts";
import { perf_report_read } from "./view.ts";
import * as input from "@/service/core/input.ts";
import * as perf from "@/service/core/perf.ts";
import * as setting from "@/service/core/setting.ts";
import { surface_box } from "@/service/platform/surface.tsx";

/** One input is sent each step, about one frame at 60 Hz. */
const STEP_MS = 16;
/** `WHEEL` is one wheel step's `deltaY`, and `RUN` is the number of steps
 * that take the zoom from one end to the other. */
const WHEEL = 100;
const RUN = 60;
/** How far the drag up goes that tilts the default view to just over the
 * horizon, as a share of the canvas's height. */
const TILT_SHARE = 0.06;
/** The pointer turns the view by moving round a circle. `RADIUS` is the
 * circle's radius in CSS pixels, and `TURN_S` the seconds one turn takes. */
const RADIUS = 250;
const TURN_S = 4;
/** The time the settings and the framing are given to take effect before
 * the window starts. */
const SETTLE_MS = 2000;
/** The seconds the window lasts. */
const DRIVE_S = 20;
/** The pointer the drive turns the view with: a mouse's secondary button. */
const POINTER_ID = 1;
const BUTTON = 2;

/** What the drive sets, each setting with its value, so a build made for
 * measuring can put them back (perf/measure.ts). */
export const DRIVE_SETTINGS: readonly (readonly [number, number])[] = [
  [SETTING.res.key, SETTING.res.options.fixed],
  [SETTING.shadow.key, 1],
  [SETTING.perf.key, 1],
  [SETTING.sky_time.key, SETTING.sky_time.options.dawn],
  [SETTING.orbit_obstacle.key, SETTING.orbit_obstacle.options.through],
];

const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

export async function perf_drive(): Promise<string> {
  const box = surface_box();
  if (!box) {
    return "no canvas";
  }
  for (const [key, value] of DRIVE_SETTINGS) setting.set(key, value);
  const tilt = TILT_SHARE * box.height;
  const [x, y] = [box.width / 2, box.height / 2];
  const pointer = (phase: PointerPhase, at: [f64, f64]) =>
    input.pointer(phase, POINTER_TYPE_MOUSE, POINTER_ID, BUTTON, at[0], at[1]);
  for (let n = 0; n < RUN; n++) {
    input.wheel(WHEEL);
    await wait(STEP_MS);
  }
  pointer(POINTER_PHASE_DOWN, [x, y]);
  for (let n = 1; n <= RUN; n++) {
    pointer(POINTER_PHASE_MOVE, [x, y - (tilt * n) / RUN]);
    await wait(STEP_MS);
  }
  pointer(POINTER_PHASE_UP, [x, y - tilt]);
  await wait(SETTLE_MS);
  perf.start();
  const on = (a: number): [f64, f64] => [
    x + RADIUS * Math.cos(a),
    y + RADIUS * Math.sin(a),
  ];
  const start = -Math.PI / 2;
  pointer(POINTER_PHASE_DOWN, on(start));
  const t0 = performance.now();
  // The sweep's place, in steps in from the farthest zoom, is computed from
  // the clock. A late frame sends every step it owes, so a fast build and a
  // slow one pass the same zooms at the same seconds.
  const leg = RUN * STEP_MS;
  let swept = 0;
  const step = setInterval(() => {
    const ms = performance.now() - t0;
    pointer(POINTER_PHASE_MOVE, on(start + (ms / 1000 / TURN_S) * 2 * Math.PI));
    const part = (ms % leg) / leg;
    const steps = RUN * (Math.floor(ms / leg) % 2 ? 1 - part : part);
    input.wheel((swept - steps) * WHEEL);
    swept = steps;
  }, STEP_MS);
  await wait(DRIVE_S * 1000);
  clearInterval(step);
  pointer(POINTER_PHASE_UP, on(start));
  return perf_report_text(await perf_report_read());
}
