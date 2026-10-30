// @pxl/client/driver/sensor.ts

/** The device's sensors as the kernel's commands: what each platform's driver
 * heard, in its words (bracket.tsx `SensorHeard`), sent as the kernel numbers
 * it; and the kernel's asks that one start or stop, passed to the place's. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  encode_sensor_failed,
  encode_sensor_orient,
  encode_sensor_permission,
  encode_sensor_pos,
  GeolocationErrorCode,
  PermissionAnswer,
  read_sensor_start,
  read_sensor_stop,
  Sensor,
} from "@/abi/cmd.spec.ts";
import { bracket, type SensorHeard } from "@/bracket.tsx";
import { HOST } from "@/host/mount.ts";
import * as port from "@/abi/port.spec.ts";

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
  HOST.send(
    port.SHELL,
    Command.SENSOR_POS,
    encode_sensor_pos(
      HOST.doorbell,
      lon,
      lat,
      given(alt),
      given(accuracy),
      given(course),
    ),
  );
}

function kernel_pos_err(code: GeolocationErrorCode): void {
  HOST.send(
    port.SHELL,
    Command.SENSOR_FAILED,
    encode_sensor_failed(HOST.doorbell, Sensor.Geolocation, code),
  );
}

function kernel_orient(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
  beta: f64 | null,
  gamma: f64 | null,
): void {
  HOST.send(
    port.SHELL,
    Command.SENSOR_ORIENT,
    encode_sensor_orient(
      HOST.doorbell,
      given(webkit_compass_heading),
      given(alpha),
      given(beta),
      given(gamma),
    ),
  );
}

/** The compass gives no code for its failure. */
function kernel_orient_lost(): void {
  HOST.now(
    port.SHELL,
    Command.SENSOR_FAILED,
    encode_sensor_failed(HOST.doorbell, Sensor.Orientation, null),
  );
}

function kernel_permission(sensor: Sensor, answer: PermissionAnswer): void {
  HOST.send(
    port.SHELL,
    Command.SENSOR_PERMISSION,
    encode_sensor_permission(HOST.doorbell, sensor, answer),
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
  positionError: (error) => kernel_pos_err(ERR[error]),
  orientation: kernel_orient,
  orientationLost: kernel_orient_lost,
  permission: (sensor, answer) =>
    kernel_permission(SENSOR[sensor], ANSWER[answer]),
};

/** The kernel's SENSOR_START. */
export function sensor_start(f: BranchCursor): void {
  if (read_sensor_start(f).sensor === Sensor.Geolocation) {
    bracket.sensor.geo.start();
  } else bracket.sensor.orient.start();
}

/** The kernel's SENSOR_STOP. */
export function sensor_stop(f: BranchCursor): void {
  if (read_sensor_stop(f).sensor === Sensor.Geolocation) {
    bracket.sensor.geo.stop();
  } else bracket.sensor.orient.stop();
}
