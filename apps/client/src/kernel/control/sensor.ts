// @pxl/client/kernel/control/sensor.ts

/** The host's end of the device's sensors: each platform's driver reports what it hears through these. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import {
  Command,
  encode_sensor_error,
  encode_sensor_orientation,
  encode_sensor_permission,
  encode_sensor_position,
  type GeolocationErrorCode,
  type PermissionAnswer,
  type Sensor,
} from "@/kernel/abi/cmd.spec.ts";
import { GUEST_DOORBELL } from "@/kernel/mem.ts";
import { ring_now, send_next } from "@/kernel/sched.ts";

/** A device reading the device could not give — NaN, an infinity — is sent as none. */
function given(n: number | null): number | null {
  return n !== null && Number.isFinite(n) ? n : null;
}

export function kernel_position(lon: f64, lat: f64, altitude: f64 | null, accuracy: f64 | null, course: f64 | null): void {
  send_next(Command.SENSOR_POSITION, encode_sensor_position(GUEST_DOORBELL, lon, lat, given(altitude), given(accuracy), given(course)));
}

export function kernel_position_error(code: GeolocationErrorCode): void {
  send_next(Command.SENSOR_ERROR, encode_sensor_error(GUEST_DOORBELL, code));
}

export function kernel_orientation(webkit_compass_heading: f64 | null, alpha: f64 | null, beta: f64 | null, gamma: f64 | null): void {
  send_next(Command.SENSOR_ORIENTATION, encode_sensor_orientation(GUEST_DOORBELL, given(webkit_compass_heading), given(alpha), given(beta), given(gamma)));
}

export function kernel_orientation_lost(): void {
  ring_now(Command.SENSOR_ORIENTATION_LOST);
}

export function kernel_permission(sensor: Sensor, answer: PermissionAnswer): void {
  send_next(Command.SENSOR_PERMISSION, encode_sensor_permission(GUEST_DOORBELL, sensor, answer));
}
