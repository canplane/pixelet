// @pxl/client/port/sensor.ts

/** SENSOR on the UI's port: what the place's sensors heard, in its words
 * (service/types.ts `SensorListener`), sent as the core numbers it; the core's
 * asks that one start or stop, passed to the place's; and the location the
 * search panel's card asks for. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  GeolocationErrorCode,
  PermissionAnswer,
  Sensor,
  type SensorStart,
  type SensorStop,
} from "@/abi/cmd.spec.ts";
import { type SensorListener } from "@/service/types.ts";
import * as sensor from "@/service/sensor.ts";
import { core } from "@/port/core.ts";

export function locate(): void {
  core.sensor_locate();
}

/** A device reading the device could not give — NaN, an infinity — is sent as none. */
function given(n: number | null): number | null {
  return n !== null && Number.isFinite(n) ? n : null;
}

function core_pos(
  lon: f64,
  lat: f64,
  alt: f64 | null,
  accuracy: f64 | null,
  course: f64 | null,
): void {
  core.sensor_pos(lon, lat, given(alt), given(accuracy), given(course));
}

function core_orient(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
  beta: f64 | null,
  gamma: f64 | null,
): void {
  core.sensor_orient(
    given(webkit_compass_heading),
    given(alpha),
    given(beta),
    given(gamma),
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

/** What the place's sensors heard, handed to it once at boot. */
export const SENSOR_LISTENER: SensorListener = {
  position: core_pos,
  positionError: (error) => core.sensor_err(Sensor.Geolocation, ERR[error]),
  orientation: core_orient,
  // The compass gives no code for its failure.
  orientationLost: () => core.sensor_err(Sensor.Orientation, null),
  permission: (sensor, answer) =>
    core.sensor_permission(SENSOR[sensor], ANSWER[answer]),
};

/** The core's SENSOR_START. */
export function sensor_start(e: SensorStart): void {
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.start();
  } else sensor.orient.start();
}

/** The core's SENSOR_STOP. */
export function sensor_stop(e: SensorStop): void {
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.stop();
  } else sensor.orient.stop();
}
