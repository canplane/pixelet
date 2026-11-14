// @pxl/client/abi/types.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import {
  type ErrorCode,
  type Sensor,
  of_ErrorCode,
  of_Sensor,
} from "./types.enum.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift, Lower } from "@xpute/link/cabi.ts";

export function load_Sensor(r: Lift, at: number): Sensor {
  const v = of_Sensor(r.case(at, 2));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export function load_ErrorCode(r: Lift, at: number): ErrorCode {
  const v = of_ErrorCode(r.case(at, 6));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export interface Shard {
  name: string;
  versions: (string | null)[];
}

export const SHARD_SZ = 16;
export const SHARD_ALIGN = 4;

export function store_Shard(w: Lower, at: number, v: Shard): void {
  w.str(at, v.name);
  {
    const b0 = w.list(at + 8, v.versions.length, 12, 4);
    if (b0 !== null)
      v.versions.forEach((x0, i0) => {
        if (x0 !== null) {
          w.u8(b0 + i0 * 12, 1);
          w.str(b0 + i0 * 12 + 4, x0);
        }
      });
  }
}

export function load_Shard(r: Lift, at: number): Shard {
  return {
    name: r.str(at),
    versions: r.map(at + 8, 12, 4, (at) =>
      r.case(at, 2) === 0 ? null : r.str(at + 4),
    ),
  };
}

export interface Declaration {
  min_zoom: number;
  formats: Uint32Array<ArrayBuffer>;
  versions: string[];
  shards: Shard[];
}

export const DECLARATION_SZ = 28;
export const DECLARATION_ALIGN = 4;

export function store_Declaration(w: Lower, at: number, v: Declaration): void {
  w.u32(at, v.min_zoom);
  w.arr(at + 4, v.formats);
  {
    const b0 = w.list(at + 12, v.versions.length, 8, 4);
    if (b0 !== null)
      v.versions.forEach((x0, i0) => {
        w.str(b0 + i0 * 8, x0);
      });
  }
  {
    const b0 = w.list(at + 20, v.shards.length, SHARD_SZ, SHARD_ALIGN);
    if (b0 !== null)
      v.shards.forEach((x0, i0) => {
        store_Shard(w, b0 + i0 * SHARD_SZ, x0);
      });
  }
}

export function load_Declaration(r: Lift, at: number): Declaration {
  return {
    min_zoom: r.u32(at),
    formats: r.arr(at + 4, Uint32Array),
    versions: r.map(at + 12, 8, 4, (at) => r.str(at)),
    shards: r.map(at + 20, SHARD_SZ, SHARD_ALIGN, (at) => load_Shard(r, at)),
  };
}
