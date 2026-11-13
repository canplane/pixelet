// @pxl/client/abi/camera.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Sender, type Handle } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/camera` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x5a7108;

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
  for (const k in Tracking)
    if (Tracking[k as keyof typeof Tracking] === n)
      return k as keyof typeof Tracking;
  return undefined;
}

/** The `Tracking` member `n` is, or null for a number no member has. */
export function tracking_of(n: number): Tracking | null {
  return tracking_name(n) === undefined ? null : (n as Tracking);
}

export async function track(link_: Sender, tracking: Tracking): Promise<void> {
  const a_ = await link_.send(HANDLE, TRACK, [
    1,
    (w_) => {
      w_.u32(tracking);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function north_up(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, NORTH_UP, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function goto(
  link_: Sender,
  lon: number,
  lat: number,
  dist: number | null,
  azimuth: number | null,
  polar: number | null,
): Promise<void> {
  const a_ = await link_.send(HANDLE, GOTO, [
    5,
    (w_) => {
      w_.f64(lon);
      w_.f64(lat);
      if (dist === null) w_.nil();
      else {
        w_.f64(dist);
      }
      if (azimuth === null) w_.nil();
      else {
        w_.f64(azimuth);
      }
      if (polar === null) w_.nil();
      else {
        w_.f64(polar);
      }
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}
