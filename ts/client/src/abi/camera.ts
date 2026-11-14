// @pxl/client/abi/camera.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { type Tracking, of_Tracking } from "./camera.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { MarshalError, XputeError } from "@xpute/kit/status/error.ts";
import type { Lift } from "@xpute/link/cabi.ts";
import type { Handle, Method, Sender } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/camera` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x5a7108;

/** `track` */
export const TRACK: Method = 0;
/** `north-up` */
export const NORTH_UP: Method = 1;
/** `goto` */
export const GOTO: Method = 2;

export function load_Tracking(r: Lift, at: number): Tracking {
  const v = of_Tracking(r.case(at, 3));
  if (v === null) throw new MarshalError(ERRNO_EBADMSG);
  return v;
}

export async function track(link_: Sender, tracking: Tracking): Promise<void> {
  const a_ = await link_.send(HANDLE, TRACK, [
    1,
    (w_) => {
      w_.u8(0, tracking);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function north_up(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, NORTH_UP, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
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
    64,
    (w_) => {
      w_.f64(0, lon);
      w_.f64(8, lat);
      if (dist !== null) {
        w_.u8(16, 1);
        w_.f64(16 + 8, dist);
      }
      if (azimuth !== null) {
        w_.u8(32, 1);
        w_.f64(32 + 8, azimuth);
      }
      if (polar !== null) {
        w_.u8(48, 1);
        w_.f64(48 + 8, polar);
      }
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
