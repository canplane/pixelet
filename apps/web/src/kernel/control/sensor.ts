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

export function kernel_position(p: SensorPosition): void {
  send_next(Command.SENSOR_POSITION, encode_sensor_position(p));
}

export function kernel_position_error(p: SensorError): void {
  send_next(Command.SENSOR_ERROR, encode_sensor_error(p));
}

export function kernel_orientation(p: SensorOrientation): void {
  send_next(Command.SENSOR_ORIENTATION, encode_sensor_orientation(p));
}

export function kernel_orientation_lost(): void {
  ring_now(Command.SENSOR_ORIENTATION_LOST);
}

export function kernel_permission(p: SensorPermission): void {
  send_next(Command.SENSOR_PERMISSION, encode_sensor_permission(p));
}
