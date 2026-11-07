// @pxl/client/port/camera.ts

/** The camera's commands: the tracking button, the compass, a search
 * result. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Tracking } from "@/abi/cmd.spec.ts";
import { core } from "@/port/core.ts";
import * as sensor from "@/service/sensor.ts";
import { interactive } from "./input.ts";

/** iOS Safari's compass prompt runs here, inside the press. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  interactive();
  core.camera_tracking(tracking);
}

/** The compass's press: north up. */
export function north_up(): void {
  core.camera_north_up();
}

/** A search result: the pivot moved there, the view as it is. */
export function goto(lon: f64, lat: f64): void {
  core.camera_goto(lon, lat, null, null, null);
}
