// @pxl/client/service/core/sensor.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  Command,
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

export function locate(): void {
  core.send(Command.SENSOR_LOCATE);
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
  core.send(
    Command.SENSOR_POS,
    lon,
    lat,
    finite(alt),
    finite(accuracy),
    finite(course),
  );
}

function core_orient(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
  beta: f64 | null,
  gamma: f64 | null,
): void {
  core.send(
    Command.SENSOR_ORIENT,
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
  positionError: (err) =>
    core.send(Command.SENSOR_ERR, Sensor.Geolocation, ERR[err]),
  orientation: core_orient,
  // The compass gives no code for its failure.
  orientationLost: () =>
    core.send(Command.SENSOR_ERR, Sensor.Orientation, null),
  permission: (sensor, answer) =>
    core.send(Command.SENSOR_PERMISSION, SENSOR[sensor], ANSWER[answer]),
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
