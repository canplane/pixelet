// @pxl/client/service/platform/sensor-listener.ts

import type { f64 } from "@xpute/kit/abi/word.ts";

import type { Coords, Fix, SensorListener } from "./types.ts";

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

/** Where a build made for measuring stands on every platform, so that every
 * run is on the same ground: Seoraksan, whose ridges the drive's eye meets at
 * the horizon. */
export const MEASURE_PLACE: Fix = {
  lat: 38.1193,
  lon: 128.4656,
  alt: null,
  alt_accuracy: null,
  accuracy: 10,
  heading: null,
};

/** The fix a page's or a device's position reads as. */
export function coords_fix(c: Coords): Fix {
  return {
    lon: c.longitude,
    lat: c.latitude,
    alt: c.altitude,
    alt_accuracy: c.altitudeAccuracy,
    accuracy: c.accuracy,
    heading: c.heading,
  };
}

/** An altitude without an accuracy was not measured, so it is passed on as
 * null. A desktop's location gives 0 for it. */
export function geo_fix(fix: Fix): void {
  listener.position(
    fix.lon,
    fix.lat,
    null_if_negative(fix.alt_accuracy) === null ? null : fix.alt,
    null_if_negative(fix.accuracy),
    null_if_negative(fix.heading),
  );
}
