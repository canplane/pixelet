// @pxl/client/shell/debug/perf/drive.ts

/**
 * The one measurement (`pxl.perf.drive`), the worst the renderer meets and
 * taken the same way each time: the display's pixels fixed, the shadows and
 * the profiler on, the sky at 07:00; the camera zoomed to its farthest and
 * tilted to the horizon through the canvas's own input; then, for the window,
 * turned a full circle every `TURN_S` while the wheel sweeps the zoom in and
 * out; and the window's report. Two runs alike are two windows alike, so a
 * change is read off their reports.
 */

import { kernel_setting } from "@/port/setting.ts";
import { kernel_perf_start } from "@/port/perf.ts";
import { SETTING } from "@/shell/setting/schema.ts";
import { perfReportText } from "./report.ts";
import { perfReportRead } from "./view.ts";

/** An input a step, about a frame at 60 Hz. */
const STEP_MS = 16;
/** The wheel's step, and the steps that take the zoom from one end to the other. */
const WHEEL = 100;
const RUN = 60;
/** The drag up that tilts the view to the horizon, in CSS pixels. */
const TILT_PX = 2000;
/** The circle the pointer turns the view on, its radius in CSS pixels and a turn's seconds. */
const RADIUS = 250;
const TURN_S = 4;
/** The settings' and the framing's time to land before the window starts. */
const SETTLE_MS = 2000;

const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

export async function perfDrive(seconds = 60): Promise<string> {
  const canvas = document.querySelector("canvas");
  if (!canvas) {
    return "no canvas";
  }
  kernel_setting(SETTING.res.key, SETTING.res.options.fixed);
  kernel_setting(SETTING.shadows.key, 1);
  kernel_setting(SETTING.perf.key, 1);
  kernel_setting(SETTING.sky_time.key, SETTING.sky_time.options.dawn);
  const r = canvas.getBoundingClientRect();
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
    pointer("pointermove", [x, y - (TILT_PX * n) / RUN], 2);
    await wait(STEP_MS);
  }
  pointer("pointerup", [x, y - TILT_PX], 0);
  await wait(SETTLE_MS);
  kernel_perf_start();
  const on = (a: number): [number, number] => [
    x + RADIUS * Math.cos(a),
    y + RADIUS * Math.sin(a),
  ];
  const start = -Math.PI / 2;
  pointer("pointerdown", on(start), 2);
  const t0 = performance.now();
  let n = 0;
  const step = setInterval(() => {
    pointer(
      "pointermove",
      on(start + ((performance.now() - t0) / 1000 / TURN_S) * 2 * Math.PI),
      2,
    );
    wheel(Math.floor(n / RUN) % 2 ? WHEEL : -WHEEL);
    n++;
  }, STEP_MS);
  await wait(seconds * 1000);
  clearInterval(step);
  pointer("pointerup", on(start), 0);
  return perfReportText(perfReportRead());
}
