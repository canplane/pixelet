// @pxl/client/service/core/camera.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { type Tracking, TRACKING_FOLLOW } from "@/abi/camera.enum.ts";
import * as camera from "@/abi/camera.ts";
import { ui } from "@/service/core/core.ts";
import * as sensor from "@/service/platform/sensor.ts";
import { ask_heading } from "@/service/core/sensor.ts";

/** iOS Safari asks for the compass only during a press, and the prompt runs
 * here, so this must be called from the press itself. Follow asks for the
 * heading, and the view turns with it once it comes. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === TRACKING_FOLLOW) {
    if (!(await sensor.ORIENT.permit())) {
      console.error("[orientation] could not start the sensor.");
      return;
    }
    ask_heading();
  }
  void camera.track(ui, tracking);
}

export function north_up(): void {
  void camera.north_up(ui);
}

/** Moves the pivot to a search result, and keeps the rest of the view as it
 * is. */
export function goto(lon: f64, lat: f64): void {
  void camera.goto(ui, lon, lat, null, null, null);
}
