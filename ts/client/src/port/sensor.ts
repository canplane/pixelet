// @pxl/client/port/sensor.ts

/** SENSOR on the shell's port: what the place's sensors heard, in its words
 * (services/types.ts `SensorHeard`), sent as the kernel numbers it; the kernel's
 * asks that one start or stop, passed to the place's; and the location the
 * search panel's card asks for. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  GeolocationErrorCode,
  PermissionAnswer,
  Sensor,
  type SensorStartEvent,
  type SensorStopEvent,
} from "@/abi/cmd.spec.ts";
import { type SensorHeard } from "@/services/types.ts";
import * as sensor from "@/services/sensor.ts";
import { kernel } from "@/host/kernel.ts";

export function locate(): void {
  kernel.sensor_locate();
}

/** A device reading the device could not give — NaN, an infinity — is sent as none. */
function given(n: number | null): number | null {
  return n !== null && Number.isFinite(n) ? n : null;
}

function kernel_pos(
  lon: f64,
  lat: f64,
  alt: f64 | null,
  accuracy: f64 | null,
  course: f64 | null,
): void {
  kernel.sensor_pos(lon, lat, given(alt), given(accuracy), given(course));
}

function kernel_orient(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
  beta: f64 | null,
  gamma: f64 | null,
): void {
  kernel.sensor_orient(
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
export const SENSOR_HEARD: SensorHeard = {
  position: kernel_pos,
  positionError: (error) => kernel.sensor_err(Sensor.Geolocation, ERR[error]),
  orientation: kernel_orient,
  // The compass gives no code for its failure.
  orientationLost: () => kernel.sensor_err(Sensor.Orientation, null),
  permission: (sensor, answer) =>
    kernel.sensor_permission(SENSOR[sensor], ANSWER[answer]),
};

/** The kernel's SENSOR_START. */
export function sensor_start(e: SensorStartEvent): void {
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.start();
  } else sensor.orient.start();
}

/** The kernel's SENSOR_STOP. */
export function sensor_stop(e: SensorStopEvent): void {
  if (e.sensor === Sensor.Geolocation) {
    sensor.geo.stop();
  } else sensor.orient.stop();
}
