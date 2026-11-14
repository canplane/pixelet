// @pxl/client/service/core/sensor.ts

import type { f64 } from "@xpute/kit/abi/word.ts";
import { SENSOR_GEOLOCATION, SENSOR_ORIENT } from "@/abi/types.enum.ts";
import { type SensorListener } from "@/service/platform/types.ts";
import * as platform from "@/service/platform/sensor.ts";
import * as sensor from "@/abi/sensor.ts";
import {
  GEOLOCATION_ERROR_CODE_PERMISSION_DENIED,
  GEOLOCATION_ERROR_CODE_POSITION_UNAVAILABLE,
  GEOLOCATION_ERROR_CODE_TIMEOUT,
  GEOLOCATION_ERROR_CODE_OTHER,
  PERMISSION_ANSWER_ASKING,
  PERMISSION_ANSWER_GRANTED,
  PERMISSION_ANSWER_DENIED,
  PERMISSION_ANSWER_FAILED,
} from "@/abi/sensor.enum.ts";
import { ui } from "@/service/core/core.ts";

/** Starts the location once it is permitted. It must run inside a press,
 * which raises the permission's prompt or its settings. */
export async function locate(): Promise<void> {
  if (await platform.GEOLOCATION.permit()) void sensor.locate(ui);
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
  void sensor.pos(ui, lon, lat, finite(alt), finite(accuracy), finite(course));
}

function core_orient(
  webkit_compass_heading: f64 | null,
  alpha: f64 | null,
): void {
  void sensor.orient(ui, finite(webkit_compass_heading), finite(alpha));
}

const ERR = {
  denied: GEOLOCATION_ERROR_CODE_PERMISSION_DENIED,
  unavailable: GEOLOCATION_ERROR_CODE_POSITION_UNAVAILABLE,
  timeout: GEOLOCATION_ERROR_CODE_TIMEOUT,
  other: GEOLOCATION_ERROR_CODE_OTHER,
} as const;

const SENSOR = {
  geolocation: SENSOR_GEOLOCATION,
  orient: SENSOR_ORIENT,
} as const;

const ANSWER = {
  asking: PERMISSION_ANSWER_ASKING,
  granted: PERMISSION_ANSWER_GRANTED,
  denied: PERMISSION_ANSWER_DENIED,
  failed: PERMISSION_ANSWER_FAILED,
} as const;

export const SENSOR_LISTENER: SensorListener = {
  pos: core_pos,
  pos_err: (err, watching) =>
    void sensor.err(ui, SENSOR_GEOLOCATION, ERR[err], watching),
  orient: core_orient,
  orient_unavailable: () => void sensor.orient_unavailable(ui),
  // The compass gives no code for its failure.
  orient_err: () => void sensor.err(ui, SENSOR_ORIENT, null, false),
  permission: (which, answer) =>
    void sensor.permission(ui, SENSOR[which], ANSWER[answer]),
};

/** Tells the core which sensors the device has, once its interfaces are
 * open. */
export function tell_platform(): Promise<void> {
  return sensor.platform(
    ui,
    platform.GEOLOCATION.has,
    platform.ORIENT.has,
    platform.ORIENT.gated,
    platform.ORIENT.silence,
  );
}
