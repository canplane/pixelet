// @pxl/client/service/sensor-listener.ts

/**
 * What a platform's sensors (sensor.ts, sensor.native.ts) tell, the same on
 * each: the listener, and a fix in its words.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";

import type { Fix, SensorListener } from "./types.ts";

/** Set once at boot, before any sensor starts. */
export let listener: SensorListener;

export function listen(given: SensorListener): void {
  listener = given;
}

/** CoreLocation says "no course" and "no accuracy" as a negative, the page's API as null. */
function null_if_negative(v: f64 | null): f64 | null {
  return v !== null && v >= 0 ? v : null;
}

/** An altitude with no accuracy is none measured: a desktop's location gives 0. */
export function geo_fix(fix: Fix): void {
  listener.position(
    fix.longitude,
    fix.latitude,
    null_if_negative(fix.altitudeAccuracy) === null ? null : fix.altitude,
    null_if_negative(fix.accuracy),
    null_if_negative(fix.heading),
  );
}
