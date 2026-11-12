// @pxl/client/service/core/camera.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Tracking } from "@/abi/camera.ts";
import * as camera from "@/abi/camera.ts";
import { core } from "@/service/core/core.ts";
import * as sensor from "@/service/platform/sensor.ts";

/** iOS Safari asks for the compass only during a press, and the prompt runs
 * here, so this must be called from the press itself. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  void camera.track(core.camera, tracking);
}

export function north_up(): void {
  void camera.north_up(core.camera);
}

/** Moves the pivot to a search result, and keeps the rest of the view as it
 * is. */
export function goto(lon: f64, lat: f64): void {
  void camera.goto(core.camera, lon, lat, null, null, null);
}
