// @pxl/client/port/camera.ts

/** The camera's commands: the tracking button, the compass, a search
 * result. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Tracking } from "@/abi/cmd.spec.ts";
import { kernel } from "@/host/kernel.ts";
import * as sensor from "@/services/sensor.ts";
import { interactive } from "./input.ts";

/** iOS Safari's compass prompt runs here, inside the press. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  interactive();
  kernel.camera_tracking(tracking);
}

/** The compass's press: north up. */
export function north_up(): void {
  kernel.camera_north_up();
}

/** A search result: the pivot moved there, the view as it is. */
export function goto(lon: f64, lat: f64): void {
  kernel.camera_goto(lon, lat, null, null, null);
}
