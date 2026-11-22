// @pxl/client/abi/camera.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { LocationMode } from "./types.enum.ts";
import {
  type Handle,
  type Method,
  type Sender,
  answer,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/camera` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x5a7108;

/** `set-location-mode` */
export const SET_LOCATION_MODE: Method = 0;
/** `goto` */
export const GOTO: Method = 1;

export async function set_location_mode(
  link_: Sender,
  mode: LocationMode,
): Promise<void> {
  const a_ = await link_.send(HANDLE, SET_LOCATION_MODE, [
    1,
    (w_) => {
      w_.u8(0, mode);
    },
  ]);
  answer(a_);
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
  answer(a_);
}
