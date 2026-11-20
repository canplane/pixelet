// @pxl/client/abi/camera.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { TrackingMode } from "./types.enum.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Handle, Method, Sender } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/camera` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x5a7108;

/** `track` */
export const TRACK: Method = 0;
/** `goto` */
export const GOTO: Method = 1;

export async function track(link_: Sender, mode: TrackingMode): Promise<void> {
  const a_ = await link_.send(HANDLE, TRACK, [
    1,
    (w_) => {
      w_.u8(0, mode);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function goto(
  link_: Sender,
  lon: number,
  lat: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, GOTO, [
    16,
    (w_) => {
      w_.f64(0, lon);
      w_.f64(8, lat);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
