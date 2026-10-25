// @pxl/client/host/abi/control/sensor.ts

/** The device's sensors as the kernel's commands: what each platform's driver
 * heard, in its words (bracket.tsx `SensorHeard`), sent as the kernel numbers it. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import { Command, encode_sensor_error, encode_sensor_orientation, encode_sensor_permission, encode_sensor_position, GeolocationErrorCode, PermissionAnswer, Sensor } from "@/host/abi/cmd.spec.ts";
import type { SensorHeard } from "@/bracket.tsx";
import { GUEST_DOORBELL, HOST } from "@/host/mem.ts";

/** A device reading the device could not give — NaN, an infinity — is sent as none. */
function given(n: number | null): number | null {
  return n !== null && Number.isFinite(n) ? n : null;
}

function kernel_position(lon: f64, lat: f64, altitude: f64 | null, accuracy: f64 | null, course: f64 | null): void {
  HOST.send(Command.SENSOR_POSITION, encode_sensor_position(GUEST_DOORBELL, lon, lat, given(altitude), given(accuracy), given(course)));
}

function kernel_position_error(code: GeolocationErrorCode): void {
  HOST.send(Command.SENSOR_ERROR, encode_sensor_error(GUEST_DOORBELL, code));
}

function kernel_orientation(webkit_compass_heading: f64 | null, alpha: f64 | null, beta: f64 | null, gamma: f64 | null): void {
  HOST.send(Command.SENSOR_ORIENTATION, encode_sensor_orientation(GUEST_DOORBELL, given(webkit_compass_heading), given(alpha), given(beta), given(gamma)));
}

function kernel_orientation_lost(): void {
  HOST.now(Command.SENSOR_ORIENTATION_LOST);
}

function kernel_permission(sensor: Sensor, answer: PermissionAnswer): void {
  HOST.send(Command.SENSOR_PERMISSION, encode_sensor_permission(GUEST_DOORBELL, sensor, answer));
}

const ERROR = {
  denied: GeolocationErrorCode.PermissionDenied,
  unavailable: GeolocationErrorCode.PositionUnavailable,
  timeout: GeolocationErrorCode.Timeout,
  other: GeolocationErrorCode.Other,
} as const;

const SENSOR = { geolocation: Sensor.Geolocation, orientation: Sensor.Orientation } as const;

const ANSWER = {
  asking: PermissionAnswer.Asking,
  granted: PermissionAnswer.Granted,
  denied: PermissionAnswer.Denied,
  failed: PermissionAnswer.Failed,
} as const;

/** What the place's sensors heard, handed to it once at boot. */
export const SENSOR_HEARD: SensorHeard = {
  position: kernel_position,
  positionError: (error) => kernel_position_error(ERROR[error]),
  orientation: kernel_orientation,
  orientationLost: kernel_orientation_lost,
  permission: (sensor, answer) => kernel_permission(SENSOR[sensor], ANSWER[answer]),
};
