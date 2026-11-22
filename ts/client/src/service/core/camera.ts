// @pxl/client/service/core/camera.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { type LocationMode, LOCATION_MODE_TRACK } from "@/abi/types.enum.ts";
import * as camera from "@/abi/camera.ts";
import { ui } from "@/service/core/core.ts";
import { startOrientation } from "@/service/core/sensor.ts";

/** Asks for a location mode. Tracking starts the heading first, and the view
 * turns with it where there is one. On the web the compass's permission may
 * be asked only during a press, so this must be called from the press
 * itself. */
export async function setLocationMode(mode: LocationMode): Promise<void> {
  if (mode === LOCATION_MODE_TRACK) await startOrientation();
  void camera.set_location_mode(ui, mode);
}

/** Sends the camera to a search result. Its facing stays, and a view farther
 * out than the default comes in to it on the way. */
export function goto(lon: f64, lat: f64): void {
  void camera.goto(ui, lon, lat);
}
