// @pxl/client/abi/sensor.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type GeolocationErrorCode,
  type PermissionAnswer,
  of_GeolocationErrorCode,
  of_PermissionAnswer,
} from "./sensor.enum.ts";
import type { Sensor } from "./types.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";
import type { Handle, Method, Sender } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/sensor` (frame.json5 `NAMED`). */
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

export function load_GeolocationErrorCode(
  r: Lift,
  at: number,
): GeolocationErrorCode {
  const v = of_GeolocationErrorCode(r.case(at, 4));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export function load_PermissionAnswer(r: Lift, at: number): PermissionAnswer {
  const v = of_PermissionAnswer(r.case(at, 4));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
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
    64,
    (w_) => {
      w_.f64(0, lon);
      w_.f64(8, lat);
      if (alt !== null) {
        w_.u8(16, 1);
        w_.f64(24, alt);
      }
      if (accuracy !== null) {
        w_.u8(32, 1);
        w_.f64(40, accuracy);
      }
      if (course !== null) {
        w_.u8(48, 1);
        w_.f64(56, course);
      }
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function orient(
  link_: Sender,
  webkit_compass_heading: number | null,
  alpha: number | null,
): Promise<void> {
  const a_ = await link_.send(HANDLE, ORIENT, [
    32,
    (w_) => {
      if (webkit_compass_heading !== null) {
        w_.u8(0, 1);
        w_.f64(8, webkit_compass_heading);
      }
      if (alpha !== null) {
        w_.u8(16, 1);
        w_.f64(24, alpha);
      }
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function err(
  link_: Sender,
  sensor: Sensor,
  code: GeolocationErrorCode | null,
  watching: boolean,
): Promise<void> {
  const a_ = await link_.send(HANDLE, ERR, [
    4,
    (w_) => {
      w_.u8(0, sensor);
      if (code !== null) {
        w_.u8(1, 1);
        w_.u8(2, code);
      }
      w_.bool(3, watching);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function permission(
  link_: Sender,
  sensor: Sensor,
  answer: PermissionAnswer,
): Promise<void> {
  const a_ = await link_.send(HANDLE, PERMISSION, [
    2,
    (w_) => {
      w_.u8(0, sensor);
      w_.u8(1, answer);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
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
      w_.bool(0, geolocation);
      w_.bool(1, orient);
      w_.bool(2, orient_permission);
      w_.bool(3, orient_silence_is_loss);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function locate(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, LOCATE, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
