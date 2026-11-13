// @pxl/client/abi/camera.ts
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

/** The interface's handle, named by `pxl:core/camera` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x5a7108;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "camera");

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

/** The name of the `Tracking` member numbered `n`, or undefined for a number
 * no member has. */
export function tracking_name(n: number): keyof typeof Tracking | undefined {
  return (Object.keys(Tracking) as (keyof typeof Tracking)[]).find(
    (k) => Tracking[k] === n,
  );
}

/** The `Tracking` member `n` is, or null for a number no member has. */
export function tracking_of(n: number): Tracking | null {
  return tracking_name(n) === undefined ? null : (n as Tracking);
}

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

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Camera): void {
  l.mount_named(HANDLE, object(imp));
}

export async function track(link: Sender, tracking: Tracking): Promise<void> {
  const a = await link.send(HANDLE, TRACK, [tracking]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function north_up(link: Sender): Promise<void> {
  const a = await link.send(HANDLE, NORTH_UP, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function goto(
  link: Sender,
  lon: number,
  lat: number,
  dist: number | null,
  azimuth: number | null,
  polar: number | null,
): Promise<void> {
  const a = await link.send(HANDLE, GOTO, [lon, lat, dist, azimuth, polar]);
  if (a.result < 0) throw new XputeError(a.result);
}
