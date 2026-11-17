// @pxl/client/ui/feature/debug/perf/drive.ts

/**
 * The one standard measurement (`pxl.perf.drive`). It puts the renderer
 * through the worst case it meets, the same way every time. The display's
 * pixels are fixed, the shadows and the profiler are on, the sky is at 07:00,
 * and the camera passes through the ground, so that the eye takes the same
 * path wherever it stands. Through the core's own input, held as a hand holds
 * it, the camera is zoomed to its farthest and tilted to just over the
 * horizon. Then, for the whole window, the view turns a full circle every
 * `TURN_S` seconds while the wheel sweeps the zoom in and out, both paced by
 * the clock. The result is the window's report.
 * Two runs of one build give two matching windows, so a change shows as the
 * difference between their reports.
 */

import { SETTING } from "@/abi/schema.spec.ts";
import { perfReportText } from "./report.ts";
import { perfReportRead, perfWindowStart } from "./view.ts";
import * as input from "@/service/core/input.ts";
import * as setting from "@/service/core/setting.ts";
import { surfaceBox } from "@/service/platform/surface.tsx";

/** One input is sent each step, about one frame at 60 Hz. */
const STEP_MS = 16;
/** `WHEEL` is one wheel step's `deltaY`, and `RUN` is the number of steps
 * that take the zoom from one end to the other. */
const WHEEL = 100;
const RUN = 60;
/** The tilt that takes the default view to just over the horizon, in
 * radians. */
const TILT_RAD = 0.12 * Math.PI;
/** The seconds one full turn takes. */
const TURN_S = 4;
/** The time the settings and the framing are given to take effect before
 * the window starts. */
const SETTLE_MS = 2000;
/** The seconds the window lasts. */
const DRIVE_S = 20;

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

export async function perfDrive(): Promise<string> {
  const box = surfaceBox();
  if (!box) {
    return "no canvas";
  }
  for (const [key, value] of DRIVE_SETTINGS) setting.set(key, value);
  for (let n = 0; n < RUN; n++) {
    input.wheel(WHEEL);
    await wait(STEP_MS);
  }
  input.setDragMode("turn");
  for (let n = 1; n <= RUN; n++) {
    input.tilt(TILT_RAD / RUN);
    await wait(STEP_MS);
  }
  input.setDragMode("none");
  await wait(SETTLE_MS);
  perfWindowStart();
  input.setDragMode("turn");
  const t0 = performance.now();
  let turned = 0;
  // The sweep's place, in steps in from the farthest zoom, is computed from
  // the clock. A late frame sends every step it owes, so a fast build and a
  // slow one pass the same zooms at the same seconds.
  const leg = RUN * STEP_MS;
  let swept = 0;
  const step = setInterval(() => {
    const ms = performance.now() - t0;
    const turn = (ms / 1000 / TURN_S) * 2 * Math.PI;
    input.turn(turned - turn);
    turned = turn;
    const part = (ms % leg) / leg;
    const steps = RUN * (Math.floor(ms / leg) % 2 ? 1 - part : part);
    input.wheel((swept - steps) * WHEEL);
    swept = steps;
  }, STEP_MS);
  await wait(DRIVE_S * 1000);
  clearInterval(step);
  input.setDragMode("none");
  return perfReportText(await perfReportRead());
}
