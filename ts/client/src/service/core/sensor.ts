// @pxl/client/service/core/sensor.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { GeolocationErrorCode, PermissionAnswer } from "@/abi/sensor.ts";
import { Sensor } from "@/abi/types.ts";
import { type SensorListener } from "@/service/platform/types.ts";
import * as platform from "@/service/platform/sensor.ts";
import { geo_fix, MEASURE_PLACE } from "@/service/platform/sensor-listener.ts";
import * as sensor from "@/abi/sensor.ts";
import { core } from "@/service/core/core.ts";

/** Called from the press that asks for the location, which raises the
 * permission's prompt or its settings. */
export async function locate(): Promise<void> {
  if (await platform.geo.permit()) void sensor.locate(core.sensor);
}

/** A reading the device could not give, such as NaN or an infinity, is sent
 * as none. */
function finite(n: number | null): number | null {
  return n !== null && Number.isFinite(n) ? n : null;
}

function core_pos(
  lon: f64,
  lat: f64,
  alt: f64 | null,
  accuracy: f64 | null,
  course: f64 | null,
): void {
  void sensor.pos(core.sensor, lon, lat, finite(alt), finite(accuracy), finite(course));
}

function core_orient(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
  beta: f64 | null,
  gamma: f64 | null,
): void {
  void sensor.orient(core.sensor, 
    finite(webkit_compass_heading),
    finite(alpha),
    finite(beta),
    finite(gamma),
  );
}

const ERR = {
  denied: GeolocationErrorCode.PermissionDenied,
  unavailable: GeolocationErrorCode.PositionUnavailable,
  timeout: GeolocationErrorCode.Timeout,
  other: GeolocationErrorCode.Other,
} as const;

const SENSOR = {
  geolocation: Sensor.Geolocation,
  orientation: Sensor.Orientation,
} as const;

const ANSWER = {
  asking: PermissionAnswer.Asking,
  granted: PermissionAnswer.Granted,
  denied: PermissionAnswer.Denied,
  failed: PermissionAnswer.Failed,
} as const;

export const SENSOR_LISTENER: SensorListener = {
  position: core_pos,
  positionError: (err, watching) =>
    void sensor.err(core.sensor, Sensor.Geolocation, ERR[err], watching),
  orientation: core_orient,
  // The compass gives no code for its failure.
  orientationLost: () => void sensor.err(core.sensor, Sensor.Orientation, null, false),
  permission: (which, answer) =>
    void sensor.permission(core.sensor, SENSOR[which], ANSWER[answer]),
};

export function sensor_start(s: Sensor): void {
  // A build made for measuring has its place given and starts no sensor, so
  // that no permission prompt holds it on any platform.
  if (__PERF_DRIVE__) {
    if (s === Sensor.Geolocation) geo_fix(MEASURE_PLACE);
    return;
  }
  if (s === Sensor.Geolocation) {
    platform.geo.start();
  } else platform.orient.start();
}

export function sensor_stop(s: Sensor): void {
  if (s === Sensor.Geolocation) {
    platform.geo.stop();
  } else platform.orient.stop();
}
