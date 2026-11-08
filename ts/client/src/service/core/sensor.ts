// @pxl/client/service/core/sensor.ts

/** SENSOR on the UI's port. What the place's sensors heard, in the place's
 * words (service/platform/types.ts `SensorListener`), is sent in the core's
 * numbers. The core's asks to start or stop a sensor are passed to the place's
 * sensors. The search panel's card asks for the location here. */

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
import { core } from "@/service/core/core.ts";

export function locate(): void {
  core.sensor_locate();
}

/** A reading the device could not give, such as NaN or an infinity, is sent
 * as none. */
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

/** Receives what the place's sensors heard. It is handed to the place once,
 * at boot. */
export const SENSOR_LISTENER: SensorListener = {
  position: core_pos,
  positionError: (error) => core.sensor_err(Sensor.Geolocation, ERR[error]),
  orientation: core_orient,
  // The compass gives no code for its failure.
  orientationLost: () => core.sensor_err(Sensor.Orientation, null),
  permission: (sensor, answer) =>
    core.sensor_permission(SENSOR[sensor], ANSWER[answer]),
};

/** Answers the core's SENSOR_START. */
export function sensor_start(e: SensorStart): void {
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.start();
  } else sensor.orient.start();
}

/** Answers the core's SENSOR_STOP. */
export function sensor_stop(e: SensorStop): void {
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.stop();
  } else sensor.orient.stop();
}
