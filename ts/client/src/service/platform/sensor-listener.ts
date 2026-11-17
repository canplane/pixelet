// @pxl/client/service/platform/sensor-listener.ts

/** What both platforms' sensors share: the listener they report to and how a
 * reading becomes a fix. Nothing in it differs by platform, so it is one file
 * and no pair. */

import type { f64 } from "@xpute/kit/abi/word.ts";

import type { Coordinates, Fix, SensorListener } from "./types.ts";

/** Set once at boot, before any sensor starts. */
export let listener: SensorListener;

export function listen(to: SensorListener): void {
  listener = to;
}

/** CoreLocation reports "no course" and "no accuracy" as a negative number,
 * where the web API gives null. */
function nullIfNegative(v: f64 | null): f64 | null {
  return v !== null && v >= 0 ? v : null;
}

/** Where a build made for measuring stands on every platform, so that every
 * run is on the same ground: Seoraksan, whose ridges the drive's eye meets at
 * the horizon. It sits here, beside the listener the fix goes to, because the
 * driver gives it and the drive's check (ui/feature/debug/perf/measure.ts)
 * reads it. */
export const MEASURE_PLACE: Fix = {
  lat: 38.1193,
  lon: 128.4656,
  alt: null,
  altitudeAccuracy: null,
  accuracy: 10,
  heading: null,
};

/** The fix a web or a native position reads as. */
export function coordinatesFix(c: Coordinates): Fix {
  return {
    lon: c.longitude,
    lat: c.latitude,
    alt: c.altitude,
    altitudeAccuracy: c.altitudeAccuracy,
    accuracy: c.accuracy,
    heading: c.heading,
  };
}

/** An altitude without an accuracy was not measured, so it is passed on as
 * null. A desktop's location gives 0 for it. */
export function geolocationFix(fix: Fix): void {
  listener.pos(
    fix.lon,
    fix.lat,
    nullIfNegative(fix.altitudeAccuracy) === null ? null : fix.alt,
    nullIfNegative(fix.accuracy),
    nullIfNegative(fix.heading),
  );
}
