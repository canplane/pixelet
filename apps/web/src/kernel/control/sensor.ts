// @pxl/web/kernel/control/sensor.ts

/** The host's end of the device's sensors: each platform's driver reports what it hears through these. */

import { Command, encode_sensor_error, encode_sensor_permission, encode_sensor_position, type SensorError, type SensorPermission, type SensorPosition } from "@/kernel/abi/cmd.spec.ts";
import { send_next } from "@/kernel/sched.ts";

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

export function kernel_permission(p: SensorPermission): void {
  send_next(Command.SENSOR_PERMISSION, encode_sensor_permission(p));
}
