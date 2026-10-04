// @pxl/web/kernel/control/sensor.ts

/** The host's end of the device's sensors: each platform's driver reports what it hears through these. */

import {
  Command,
  encode_sensor_error,
  encode_sensor_orientation,
  encode_sensor_permission,
  encode_sensor_position,
  type SensorError,
  type SensorOrientation,
  type SensorPermission,
  type SensorPosition,
} from "@/kernel/abi/cmd.spec.ts";
import { ring_now, send_next } from "@/kernel/sched.ts";

/** A device reading the device could not give — NaN, an infinity — is sent as none. */
function given(n: number | null): number | null {
  return n !== null && Number.isFinite(n) ? n : null;
}

export function kernel_position(p: SensorPosition): void {
  const fix = { ...p, altitude: given(p.altitude), accuracy: given(p.accuracy), course: given(p.course) };
  send_next(Command.SENSOR_POSITION, encode_sensor_position(fix));
}

export function kernel_position_error(p: SensorError): void {
  send_next(Command.SENSOR_ERROR, encode_sensor_error(p));
}

export function kernel_orientation(p: SensorOrientation): void {
  const event = { webkit_compass_heading: given(p.webkit_compass_heading), alpha: given(p.alpha), beta: given(p.beta), gamma: given(p.gamma) };
  send_next(Command.SENSOR_ORIENTATION, encode_sensor_orientation(event));
}

export function kernel_orientation_lost(): void {
  ring_now(Command.SENSOR_ORIENTATION_LOST);
}

export function kernel_permission(p: SensorPermission): void {
  send_next(Command.SENSOR_PERMISSION, encode_sensor_permission(p));
}
