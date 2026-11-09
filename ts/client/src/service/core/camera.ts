// @pxl/client/service/core/camera.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Command, Tracking } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";
import * as sensor from "@/service/platform/sensor.ts";
import { interactive } from "./input.ts";

/** iOS Safari asks for the compass only during a press, and the prompt runs
 * here, so this must be called from the press itself. */
export async function tracking(tracking: Tracking): Promise<void> {
  if (tracking === Tracking.Follow && !(await sensor.orient.permit())) {
    console.error("[orientation] could not start the sensor.");
    return;
  }
  interactive();
  core.send(Command.CAMERA_TRACKING, tracking);
}

export function north_up(): void {
  core.send(Command.CAMERA_NORTH_UP);
}

/** Moves the pivot to a search result, and keeps the rest of the view as it
 * is. */
export function goto(lon: f64, lat: f64): void {
  core.send(Command.CAMERA_GOTO, lon, lat, null, null, null);
}
