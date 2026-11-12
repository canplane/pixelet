// @pxl/client/abi/camera.ts
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

/** The singleton's URI. */
export const URI = uri("pxl:core", "camera");

/** `track` */
export const TRACK: Method = 0;
/** `north-up` */
export const NORTH_UP: Method = 1;
/** `goto` */
export const GOTO: Method = 2;

export const Tracking = {
  Recenter: 0,
  Follow: 1,
  Unfollow: 2,
} as const;
export type Tracking = (typeof Tracking)[keyof typeof Tracking];

export interface Camera {
  track(tracking: Tracking): void;
  north_up(): void;
  goto(
    lon: number,
    lat: number,
    dist: number | null,
    azimuth: number | null,
    polar: number | null,
  ): void;
}

/** The object over `imp`. */
function object(imp: Camera): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case TRACK: {
            if (args === null) return Errno.EBADMSG;
            imp.track(args.at(0).get_number() as Tracking);
            return 0;
          }
          case NORTH_UP: {
            imp.north_up();
            return 0;
          }
          case GOTO: {
            if (args === null) return Errno.EBADMSG;
            imp.goto(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_opt((c) => c.get_number()),
              args.at(3).get_opt((c) => c.get_number()),
              args.at(4).get_opt((c) => c.get_number()),
            );
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
export function mount(l: Pick<Link, "mount">, imp: Camera): void {
  const o = object(imp);
  l.mount(URI, () => o);
}

/** Opens the singleton on the other side, once, at boot. */
export async function open(link: Sender): Promise<Ref> {
  return { link, handle: await open_uri(link, URI) };
}

export async function track(ref: Ref, tracking: Tracking): Promise<void> {
  const a = await ref.link.send(ref.handle, TRACK, [tracking]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function north_up(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, NORTH_UP, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function goto(
  ref: Ref,
  lon: number,
  lat: number,
  dist: number | null,
  azimuth: number | null,
  polar: number | null,
): Promise<void> {
  const a = await ref.link.send(ref.handle, GOTO, [
    lon,
    lat,
    dist,
    azimuth,
    polar,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}
