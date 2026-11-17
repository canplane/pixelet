// @pxl/client/service/core/camera.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { type Tracking, TRACKING_FOLLOW } from "@/abi/camera.enum.ts";
import * as camera from "@/abi/camera.ts";
import { ui } from "@/service/core/core.ts";
import { startOrientation } from "@/service/core/sensor.ts";

/** Follow starts the heading first, and the view turns with it once it comes;
 * a heading not permitted leaves the tracking as it was. On the web the
 * compass's permission may be asked only during a press, so this must be
 * called from the press itself. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === TRACKING_FOLLOW && !(await startOrientation())) return;
  void camera.track(ui, tracking);
}

/** Moves the pivot to a search result, and keeps the rest of the view as it
 * is. */
export function goto(lon: f64, lat: f64): void {
  void camera.goto(ui, lon, lat);
}
