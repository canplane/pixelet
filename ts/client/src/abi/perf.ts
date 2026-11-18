// @pxl/client/abi/perf.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { load_ErrorCode } from "./types.ts";
import type { u64 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Lift, type Lower } from "@xpute/link/cabi.ts";
import {
  ComponentError,
  type Handle,
  type Method,
  type Sender,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/perf` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x6be6ac;

/** `start` */
export const START: Method = 0;
/** `stop` */
export const STOP: Method = 1;
/** `stats` */
export const STATS: Method = 2;
/** `table` */
export const TABLE: Method = 3;
/** `worst` */
export const WORST: Method = 4;
/** `strtab` */
export const STRTAB: Method = 5;

export interface Rows {
  first: u64;
  end: u64;
  bytes: Uint8Array<ArrayBuffer>;
}

export const ROWS_SZ = 24;
export const ROWS_ALIGN = 8;

export function store_Rows(w: Lower, at: number, v: Rows): void {
  w.u64(at, v.first);
  w.u64(at + 8, v.end);
  w.arr(at + 16, v.bytes);
}

export function load_Rows(r: Lift, at: number): Rows {
  return {
    first: r.u64(at),
    end: r.u64(at + 8),
    bytes: r.arr(at + 16, Uint8Array),
  };
}

export async function start(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, START, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(ERRNO_EBADMSG);
  const r_ = new Lift(a_.data);
  const v_ =
    r_.case(0, 2) === 0
      ? { tag: "ok" as const }
      : { tag: "err" as const, val: load_ErrorCode(r_, 1) };
  if (v_.tag === "err") throw new ComponentError(v_.val);
  return;
}

export async function stop(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, STOP, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
}

export async function stats(
  link_: Sender,
  window_ms: number | null,
): Promise<Uint8Array<ArrayBuffer>> {
  const a_ = await link_.send(HANDLE, STATS, [
    16,
    (w_) => {
      if (window_ms !== null) {
        w_.u8(0, 1);
        w_.f64(8, window_ms);
      }
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(ERRNO_EBADMSG);
  const r_ = new Lift(a_.data);
  return r_.arr(0, Uint8Array);
}

export async function table(
  link_: Sender,
  table: number,
  from: u64,
  cnt: number,
): Promise<Rows> {
  const a_ = await link_.send(HANDLE, TABLE, [
    24,
    (w_) => {
      w_.u32(0, table);
      w_.u64(8, from);
      w_.u32(16, cnt);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(ERRNO_EBADMSG);
  const r_ = new Lift(a_.data);
  return load_Rows(r_, 0);
}

export async function worst(link_: Sender): Promise<Uint8Array<ArrayBuffer>> {
  const a_ = await link_.send(HANDLE, WORST, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(ERRNO_EBADMSG);
  const r_ = new Lift(a_.data);
  return r_.arr(0, Uint8Array);
}

export async function strtab(link_: Sender): Promise<Uint8Array<ArrayBuffer>> {
  const a_ = await link_.send(HANDLE, STRTAB, null);
  if (a_.result !== 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(ERRNO_EBADMSG);
  const r_ = new Lift(a_.data);
  return r_.arr(0, Uint8Array);
}
