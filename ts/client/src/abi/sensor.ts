// @pxl/client/abi/sensor.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { type LocationStatus, of_LocationStatus } from "./sensor.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift, Lower } from "@xpute/link/cabi.ts";
import type { Handle, Method, Sender } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/sensor` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x5534df;

/** `pos` */
export const POS: Method = 0;
/** `heading` */
export const HEADING: Method = 1;
/** `location` */
export const LOCATION: Method = 2;

export function load_LocationStatus(r: Lift, at: number): LocationStatus {
  const v = of_LocationStatus(r.case(at, 4));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export interface Position {
  lon: number;
  lat: number;
  alt: number | null;
}

export const POSITION_SZ = 32;
export const POSITION_ALIGN = 8;

export function store_Position(w: Lower, at: number, v: Position): void {
  w.f64(at, v.lon);
  w.f64(at + 8, v.lat);
  if (v.alt !== null) {
    w.u8(at + 16, 1);
    w.f64(at + 16 + 8, v.alt);
  }
}

export function load_Position(r: Lift, at: number): Position {
  return {
    lon: r.f64(at),
    lat: r.f64(at + 8),
    alt: r.case(at + 16, 2) === 0 ? null : r.f64(at + 16 + 8),
  };
}

export async function pos(link_: Sender, at: Position | null): Promise<void> {
  const a_ = await link_.send(HANDLE, POS, [
    40,
    (w_) => {
      if (at !== null) {
        w_.u8(0, 1);
        store_Position(w_, 8, at);
      }
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function heading(
  link_: Sender,
  deg: number | null,
): Promise<void> {
  const a_ = await link_.send(HANDLE, HEADING, [
    16,
    (w_) => {
      if (deg !== null) {
        w_.u8(0, 1);
        w_.f64(8, deg);
      }
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function location(
  link_: Sender,
  status: LocationStatus,
): Promise<void> {
  const a_ = await link_.send(HANDLE, LOCATION, [
    1,
    (w_) => {
      w_.u8(0, status);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
