// @pxl/client/service/platform/sensor-listener.ts

/**
 * What both platforms' sensors (sensor.ts, sensor.native.ts) share: the
 * listener they tell, and how a fix is put in the listener's terms.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";

import type { Fix, SensorListener } from "./types.ts";

/** Set once at boot, before any sensor starts. */
export let listener: SensorListener;

export function listen(given: SensorListener): void {
  listener = given;
}

/** CoreLocation reports "no course" and "no accuracy" as a negative number,
 * where the page's API gives null. */
function null_if_negative(v: f64 | null): f64 | null {
  return v !== null && v >= 0 ? v : null;
}

/** An altitude without an accuracy was not measured, so it is passed on as
 * null. A desktop's location gives 0 for it. */
export function geo_fix(fix: Fix): void {
  listener.position(
    fix.longitude,
    fix.latitude,
    null_if_negative(fix.altitudeAccuracy) === null ? null : fix.altitude,
    null_if_negative(fix.accuracy),
    null_if_negative(fix.heading),
  );
}
