// @pxl/client/service/core/camera.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  type TrackingMode,
  TRACKING_MODE_FOLLOW_WITH_HEADING,
  TRACKING_MODE_NONE,
} from "@/abi/types.enum.ts";
import * as camera from "@/abi/camera.ts";
import { ui } from "@/service/core/core.ts";
import { startOrientation } from "@/service/core/sensor.ts";

/** Sets the tracking mode. Following with the heading starts the heading
 * first, and the view turns with it once it comes; where there is no heading,
 * the platform's or the user's permission, the press ends tracking, as the
 * button goes without one. On the web the compass's permission may be asked
 * only during a press, so this must be called from the press itself. */
export async function track(mode: TrackingMode): Promise<void> {
  const heading = mode === TRACKING_MODE_FOLLOW_WITH_HEADING;
  const permitted = !heading || (await startOrientation());
  void camera.track(ui, permitted ? mode : TRACKING_MODE_NONE);
}

/** Sends the camera to a search result. Its facing stays, and a view farther
 * out than the default comes in to it on the way. */
export function goto(lon: f64, lat: f64): void {
  void camera.goto(ui, lon, lat);
}
