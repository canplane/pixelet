// @pxl/client/abi/sensor.ts
//
// GENERATED from spec/client/core/wit (xpute-gen spec/wit.rs)

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  open as open_uri,
  refusal_of,
  type Ref,
  type Sender,
  uri,
} from "@xpute/runtime";
import * as types from "./types.ts";

/** The singleton's URI. */
export const URI = uri("pxl:core", "sensor");

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

export const PermissionAnswer = {
  Asking: 0,
  Granted: 1,
  Denied: 2,
  Failed: 3,
} as const;
export type PermissionAnswer =
  (typeof PermissionAnswer)[keyof typeof PermissionAnswer];

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

/** Mounts `imp` at the URI, one object for every open. */
export function mount(l: Pick<Link, "mount">, imp: Sensor): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function pos(
  ref: Ref,
  lon: number,
  lat: number,
  alt: number | null,
  accuracy: number | null,
  course: number | null,
): Promise<void> {
  const a = await ref.link.send(ref.handle, POS, [
    lon,
    lat,
    alt,
    accuracy,
    course,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function orient(
  ref: Ref,
  webkit_compass_heading: number | null,
  alpha: number | null,
  beta: number | null,
  gamma: number | null,
): Promise<void> {
  const a = await ref.link.send(ref.handle, ORIENT, [
    webkit_compass_heading,
    alpha,
    beta,
    gamma,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function err(
  ref: Ref,
  sensor: types.Sensor,
  code: GeolocationErrorCode | null,
  watching: boolean,
): Promise<void> {
  const a = await ref.link.send(ref.handle, ERR, [sensor, code, watching]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function permission(
  ref: Ref,
  sensor: types.Sensor,
  answer: PermissionAnswer,
): Promise<void> {
  const a = await ref.link.send(ref.handle, PERMISSION, [sensor, answer]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function platform(
  ref: Ref,
  geolocation: boolean,
  orient: boolean,
  orient_permission: boolean,
  orient_silence_is_loss: boolean,
): Promise<void> {
  const a = await ref.link.send(ref.handle, PLATFORM, [
    geolocation,
    orient,
    orient_permission,
    orient_silence_is_loss,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function locate(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, LOCATE, []);
  if (a.result < 0) throw new XputeError(a.result);
}
