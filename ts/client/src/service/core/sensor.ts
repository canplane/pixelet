// @pxl/client/service/core/sensor.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  GeolocationErrorCode,
  PermissionAnswer,
  Sensor,
  type SensorStart,
  type SensorStop,
} from "@/abi/cmd.spec.ts";
import { type SensorListener } from "@/service/platform/types.ts";
import * as sensor from "@/service/platform/sensor.ts";
import { geo_fix, MEASURE_PLACE } from "@/service/platform/sensor-listener.ts";
import { core } from "@/service/core/core.ts";

/** Called from the press that asks for the location, which raises the
 * permission's prompt or its settings. */
export async function locate(): Promise<void> {
  if (await sensor.geo.permit()) core.sensor_locate();
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
  core.sensor_pos(lon, lat, finite(alt), finite(accuracy), finite(course));
}

function core_orient(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
  beta: f64 | null,
  gamma: f64 | null,
): void {
  core.sensor_orient(
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
  positionError: (err) => core.sensor_err(Sensor.Geolocation, ERR[err]),
  orientation: core_orient,
  // The compass gives no code for its failure.
  orientationLost: () => core.sensor_err(Sensor.Orientation, null),
  permission: (sensor, answer) =>
    core.sensor_permission(SENSOR[sensor], ANSWER[answer]),
};

export function sensor_start(e: SensorStart): void {
  // A build made for measuring has its place given and starts no sensor, so
  // that no permission prompt holds it on any platform.
  if (__PERF_DRIVE__) {
    if (e.sensor === Sensor.Geolocation) geo_fix(MEASURE_PLACE);
    return;
  }
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.start();
  } else sensor.orient.start();
}

export function sensor_stop(e: SensorStop): void {
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.stop();
  } else sensor.orient.stop();
}
