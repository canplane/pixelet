// @pxl/client/abi/sensor.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Sender, type Handle } from "@xpute/link/link.ts";
import * as types from "./types.ts";

/** The interface's handle, named by `pxl:core/sensor` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x5534df;

/** `pos` */
export const POS: Method = 0;
/** `orient` */
export const ORIENT: Method = 1;
/** `err` */
export const ERR: Method = 2;
/** `permission` */
export const PERMISSION: Method = 3;
/** `platform` */
export const PLATFORM: Method = 4;
/** `locate` */
export const LOCATE: Method = 5;

export const GeolocationErrorCode = {
  Other: 0,
  PermissionDenied: 1,
  PositionUnavailable: 2,
  Timeout: 3,
} as const;
export type GeolocationErrorCode =
  (typeof GeolocationErrorCode)[keyof typeof GeolocationErrorCode];

/** The name of the `GeolocationErrorCode` member numbered `n`, or undefined for a number
 * no member has. */
export function geolocation_error_code_name(
  n: number,
): keyof typeof GeolocationErrorCode | undefined {
  for (const k in GeolocationErrorCode)
    if (GeolocationErrorCode[k as keyof typeof GeolocationErrorCode] === n)
      return k as keyof typeof GeolocationErrorCode;
  return undefined;
}

/** The `GeolocationErrorCode` member `n` is, or null for a number no member has. */
export function geolocation_error_code_of(
  n: number,
): GeolocationErrorCode | null {
  return geolocation_error_code_name(n) === undefined
    ? null
    : (n as GeolocationErrorCode);
}

export const PermissionAnswer = {
  Asking: 0,
  Granted: 1,
  Denied: 2,
  Failed: 3,
} as const;
export type PermissionAnswer =
  (typeof PermissionAnswer)[keyof typeof PermissionAnswer];

/** The name of the `PermissionAnswer` member numbered `n`, or undefined for a number
 * no member has. */
export function permission_answer_name(
  n: number,
): keyof typeof PermissionAnswer | undefined {
  for (const k in PermissionAnswer)
    if (PermissionAnswer[k as keyof typeof PermissionAnswer] === n)
      return k as keyof typeof PermissionAnswer;
  return undefined;
}

/** The `PermissionAnswer` member `n` is, or null for a number no member has. */
export function permission_answer_of(n: number): PermissionAnswer | null {
  return permission_answer_name(n) === undefined
    ? null
    : (n as PermissionAnswer);
}

export async function pos(
  link_: Sender,
  lon: number,
  lat: number,
  alt: number | null,
  accuracy: number | null,
  course: number | null,
): Promise<void> {
  const a_ = await link_.send(HANDLE, POS, [
    5,
    (w_) => {
      w_.f64(lon);
      w_.f64(lat);
      if (alt === null) w_.nil();
      else {
        w_.f64(alt);
      }
      if (accuracy === null) w_.nil();
      else {
        w_.f64(accuracy);
      }
      if (course === null) w_.nil();
      else {
        w_.f64(course);
      }
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function orient(
  link_: Sender,
  webkit_compass_heading: number | null,
  alpha: number | null,
  beta: number | null,
  gamma: number | null,
): Promise<void> {
  const a_ = await link_.send(HANDLE, ORIENT, [
    4,
    (w_) => {
      if (webkit_compass_heading === null) w_.nil();
      else {
        w_.f64(webkit_compass_heading);
      }
      if (alpha === null) w_.nil();
      else {
        w_.f64(alpha);
      }
      if (beta === null) w_.nil();
      else {
        w_.f64(beta);
      }
      if (gamma === null) w_.nil();
      else {
        w_.f64(gamma);
      }
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function err(
  link_: Sender,
  sensor: types.Sensor,
  code: GeolocationErrorCode | null,
  watching: boolean,
): Promise<void> {
  const a_ = await link_.send(HANDLE, ERR, [
    3,
    (w_) => {
      w_.u32(sensor);
      if (code === null) w_.nil();
      else {
        w_.u32(code);
      }
      w_.bool(watching);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function permission(
  link_: Sender,
  sensor: types.Sensor,
  answer: PermissionAnswer,
): Promise<void> {
  const a_ = await link_.send(HANDLE, PERMISSION, [
    2,
    (w_) => {
      w_.u32(sensor);
      w_.u32(answer);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function platform(
  link_: Sender,
  geolocation: boolean,
  orient: boolean,
  orient_permission: boolean,
  orient_silence_is_loss: boolean,
): Promise<void> {
  const a_ = await link_.send(HANDLE, PLATFORM, [
    4,
    (w_) => {
      w_.bool(geolocation);
      w_.bool(orient);
      w_.bool(orient_permission);
      w_.bool(orient_silence_is_loss);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function locate(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, LOCATE, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}
