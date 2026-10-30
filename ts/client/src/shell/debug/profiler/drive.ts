// @pxl/client/shell/debug/profiler/drive.ts

/**
 * A measurement taken the same way each time (`pxl.profiler.drive`): the
 * display's pixels fixed, the shadows and the profiler on, the sky at an
 * hour, the window started again, the camera moved one way for a number of
 * seconds through the canvas's own input, and the window's report. An orbit
 * holds the pointer at the canvas's middle and swings it either way; a zoom
 * turns the wheel in and out by turns; still moves nothing. Two runs alike
 * are two windows alike, so a change is read off their reports.
 */

import { Command } from "@/abi/cmd.spec.ts";
import { kernel_command, kernel_setting } from "@/port/shell.ts";
import { SETTING } from "@/shell/setting/schema.ts";
import { profilerReportText } from "./report.ts";
import { profilerReportRead } from "./view.ts";

export type DriveMotion = "orbit" | "zoom" | "still";
export type DriveHour = keyof typeof SETTING.sky_time.options;

/** An input a step, about a frame at 60 Hz. */
const STEP_MS = 16;
/** An orbit's swing either way, in CSS pixels, and its steps a radian. */
const SWING = 300;
const PERIOD = 40;
/** A zoom's steps one way before it turns, and the wheel's step. */
const RUN = 30;
const WHEEL = 100;
/** The settings' time to land before the window starts. */
const SETTLE_MS = 2000;

const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

export async function profilerDrive(
  motion: DriveMotion = "orbit",
  seconds = 60,
  hour: DriveHour = "noon",
): Promise<string> {
  const canvas = document.querySelector("canvas");
  if (!canvas) {
    return "no canvas";
  }
  kernel_setting(SETTING.res.key, SETTING.res.options.fixed);
  kernel_setting(SETTING.shadows.key, 1);
  kernel_setting(SETTING.profiler.key, 1);
  kernel_setting(SETTING.sky_time.key, SETTING.sky_time.options[hour]);
  await wait(SETTLE_MS);
  kernel_command(Command.PROFILER_START);
  const r = canvas.getBoundingClientRect();
  const [x, y] = [r.left + r.width / 2, r.top + r.height / 2];
  const pointer = (type: string, at: number, buttons: number) =>
    canvas.dispatchEvent(
      new PointerEvent(type, {
        pointerId: 1,
        pointerType: "mouse",
        isPrimary: true,
        clientX: at,
        clientY: y,
        buttons,
        button: 0,
        bubbles: true,
        cancelable: true,
      }),
    );
  if (motion === "orbit") {
    pointer("pointerdown", x, 1);
  }
  let n = 0;
  const step = setInterval(() => {
    if (motion === "orbit") {
      pointer("pointermove", x + SWING * Math.sin(n / PERIOD), 1);
    } else if (motion === "zoom") {
      const deltaY = Math.floor(n / RUN) % 2 ? -WHEEL : WHEEL;
      canvas.dispatchEvent(
        new WheelEvent("wheel", {
          deltaY,
          clientX: x,
          clientY: y,
          bubbles: true,
          cancelable: true,
        }),
      );
    }
    n++;
  }, STEP_MS);
  await wait(seconds * 1000);
  clearInterval(step);
  if (motion === "orbit") {
    pointer("pointerup", x, 0);
  }
  return profilerReportText(profilerReportRead());
}
