// @pxl/client/ui/debug/perf/drive.ts

/**
 * The one standard measurement (`pxl.perf.drive`). It puts the renderer
 * through the worst case it meets, the same way every time. The display's
 * pixels are fixed, the shadows and the profiler are on, and the sky is at
 * 07:00. Through the canvas's own input, the camera is zoomed to its farthest
 * and tilted to just over the horizon. Then, for the whole window, the view
 * turns a full circle every `TURN_S` seconds while the wheel sweeps the zoom
 * in and out, both paced by the clock. The result is the window's report.
 * Two runs of one build give two matching windows, so a change shows as the
 * difference between their reports.
 */

import { SETTING } from "@/ui/setting/schema.ts";
import { perfReportText } from "./report.ts";
import { perfReportRead } from "./view.ts";
import * as perf from "@/port/perf.ts";
import * as setting from "@/port/setting.ts";

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

const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

export async function perfDrive(seconds = 60): Promise<string> {
  const canvas = document.querySelector("canvas");
  if (!canvas) {
    return "no canvas";
  }
  setting.set(SETTING.res.key, SETTING.res.options.fixed);
  setting.set(SETTING.shadows.key, 1);
  setting.set(SETTING.perf.key, 1);
  setting.set(SETTING.sky_time.key, SETTING.sky_time.options.dawn);
  const r = canvas.getBoundingClientRect();
  const tilt = TILT_SHARE * r.height;
  const [x, y] = [r.left + r.width / 2, r.top + r.height / 2];
  const pointer = (type: string, at: [number, number], buttons: number) =>
    canvas.dispatchEvent(
      new PointerEvent(type, {
        pointerId: 1,
        pointerType: "mouse",
        isPrimary: true,
        clientX: at[0],
        clientY: at[1],
        buttons,
        button: 2,
        bubbles: true,
        cancelable: true,
      }),
    );
  const wheel = (deltaY: number) =>
    canvas.dispatchEvent(
      new WheelEvent("wheel", {
        deltaY,
        clientX: x,
        clientY: y,
        bubbles: true,
        cancelable: true,
      }),
    );
  for (let n = 0; n < RUN; n++) {
    wheel(WHEEL);
    await wait(STEP_MS);
  }
  pointer("pointerdown", [x, y], 2);
  for (let n = 1; n <= RUN; n++) {
    pointer("pointermove", [x, y - (tilt * n) / RUN], 2);
    await wait(STEP_MS);
  }
  pointer("pointerup", [x, y - tilt], 0);
  await wait(SETTLE_MS);
  perf.start();
  const on = (a: number): [number, number] => [
    x + RADIUS * Math.cos(a),
    y + RADIUS * Math.sin(a),
  ];
  const start = -Math.PI / 2;
  pointer("pointerdown", on(start), 2);
  const t0 = performance.now();
  // The sweep's place, in steps in from the farthest zoom, is computed from
  // the clock. A late frame sends every step it owes, so a fast build and a
  // slow one pass the same zooms at the same seconds.
  const leg = RUN * STEP_MS;
  let swept = 0;
  const step = setInterval(() => {
    const ms = performance.now() - t0;
    pointer("pointermove", on(start + (ms / 1000 / TURN_S) * 2 * Math.PI), 2);
    const part = (ms % leg) / leg;
    const steps = RUN * (Math.floor(ms / leg) % 2 ? 1 - part : part);
    wheel((swept - steps) * WHEEL);
    swept = steps;
  }, STEP_MS);
  await wait(seconds * 1000);
  clearInterval(step);
  pointer("pointerup", on(start), 0);
  return perfReportText(await perfReportRead());
}
