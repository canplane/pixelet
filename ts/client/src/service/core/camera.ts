// @pxl/client/service/core/camera.ts

/** The camera's commands, for the tracking button, the compass and a search
 * result. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Tracking } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";
import * as sensor from "@/service/platform/sensor.ts";
import { interactive } from "./input.ts";

/** Sets how the camera tracks the device. iOS Safari asks for the compass
 * only during a press, and the prompt runs here, so this must be called from
 * the press itself. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  interactive();
  core.camera_tracking(tracking);
}

export function north_up(): void {
  core.camera_north_up();
}

/** Moves the pivot to a search result, and keeps the rest of the view as it
 * is. */
export function goto(lon: f64, lat: f64): void {
  core.camera_goto(lon, lat, null, null, null);
}
