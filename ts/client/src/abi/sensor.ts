// @pxl/client/abi/sensor.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "@xpute/link/link.ts";
import * as types from "./types.ts";

/** The interface's handle, named by `pxl:core/sensor` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x5534df;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "sensor");

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
  return (
    Object.keys(GeolocationErrorCode) as (keyof typeof GeolocationErrorCode)[]
  ).find((k) => GeolocationErrorCode[k] === n);
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
  return (
    Object.keys(PermissionAnswer) as (keyof typeof PermissionAnswer)[]
  ).find((k) => PermissionAnswer[k] === n);
}

/** The `PermissionAnswer` member `n` is, or null for a number no member has. */
export function permission_answer_of(n: number): PermissionAnswer | null {
  return permission_answer_name(n) === undefined
    ? null
    : (n as PermissionAnswer);
}

export interface Sensor {
  pos(
    lon: number,
    lat: number,
    alt: number | null,
    accuracy: number | null,
    course: number | null,
  ): void;
  orient(
    webkit_compass_heading: number | null,
    alpha: number | null,
    beta: number | null,
    gamma: number | null,
  ): void;
  err(
    sensor: types.Sensor,
    code: GeolocationErrorCode | null,
    watching: boolean,
  ): void;
  permission(sensor: types.Sensor, answer: PermissionAnswer): void;
  platform(
    geolocation: boolean,
    orient: boolean,
    orient_permission: boolean,
    orient_silence_is_loss: boolean,
  ): void;
  locate(): void;
}

/** The object over `imp`. */
function object(imp: Sensor): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case POS: {
            if (args === null) return Errno.EBADMSG;
            imp.pos(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_opt((c) => c.get_number()),
              args.at(3).get_opt((c) => c.get_number()),
              args.at(4).get_opt((c) => c.get_number()),
            );
            return 0;
          }
          case ORIENT: {
            if (args === null) return Errno.EBADMSG;
            imp.orient(
              args.at(0).get_opt((c) => c.get_number()),
              args.at(1).get_opt((c) => c.get_number()),
              args.at(2).get_opt((c) => c.get_number()),
              args.at(3).get_opt((c) => c.get_number()),
            );
            return 0;
          }
          case ERR: {
            if (args === null) return Errno.EBADMSG;
            imp.err(
              args.at(0).get_number() as types.Sensor,
              args.at(1).get_opt((c) => c.get_number() as GeolocationErrorCode),
              args.at(2).get_bool(),
            );
            return 0;
          }
          case PERMISSION: {
            if (args === null) return Errno.EBADMSG;
            imp.permission(
              args.at(0).get_number() as types.Sensor,
              args.at(1).get_number() as PermissionAnswer,
            );
            return 0;
          }
          case PLATFORM: {
            if (args === null) return Errno.EBADMSG;
            imp.platform(
              args.at(0).get_bool(),
              args.at(1).get_bool(),
              args.at(2).get_bool(),
              args.at(3).get_bool(),
            );
            return 0;
          }
          case LOCATE: {
            imp.locate();
            return 0;
          }
          default:
            return Errno.ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
  };
}

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Sensor): void {
  l.mount_named(HANDLE, object(imp));
}

export async function pos(
  link: Sender,
  lon: number,
  lat: number,
  alt: number | null,
  accuracy: number | null,
  course: number | null,
): Promise<void> {
  const a = await link.send(HANDLE, POS, [lon, lat, alt, accuracy, course]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function orient(
  link: Sender,
  webkit_compass_heading: number | null,
  alpha: number | null,
  beta: number | null,
  gamma: number | null,
): Promise<void> {
  const a = await link.send(HANDLE, ORIENT, [
    webkit_compass_heading,
    alpha,
    beta,
    gamma,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function err(
  link: Sender,
  sensor: types.Sensor,
  code: GeolocationErrorCode | null,
  watching: boolean,
): Promise<void> {
  const a = await link.send(HANDLE, ERR, [sensor, code, watching]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function permission(
  link: Sender,
  sensor: types.Sensor,
  answer: PermissionAnswer,
): Promise<void> {
  const a = await link.send(HANDLE, PERMISSION, [sensor, answer]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function platform(
  link: Sender,
  geolocation: boolean,
  orient: boolean,
  orient_permission: boolean,
  orient_silence_is_loss: boolean,
): Promise<void> {
  const a = await link.send(HANDLE, PLATFORM, [
    geolocation,
    orient,
    orient_permission,
    orient_silence_is_loss,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function locate(link: Sender): Promise<void> {
  const a = await link.send(HANDLE, LOCATE, []);
  if (a.result < 0) throw new XputeError(a.result);
}
