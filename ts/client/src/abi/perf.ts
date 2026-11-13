// @pxl/client/abi/perf.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { SequenceType, TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Method,
  type Sender,
  type Handle,
  get_as,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/perf` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x6be6ac;

/** `start` */
export const START: Method = 0;
/** `stop` */
export const STOP: Method = 1;
/** `stats` */
export const STATS: Method = 2;
/** `rings` */
export const RINGS: Method = 3;
/** `strtab` */
export const STRTAB: Method = 4;

export async function start(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, START, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function stop(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, STOP, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function stats(
  link_: Sender,
  window_ms: number | null,
): Promise<U8Array> {
  const a_ = await link_.send(HANDLE, STATS, [
    1,
    (w_) => {
      if (window_ms === null) w_.nil();
      else {
        w_.f64(window_ms);
      }
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(Errno.EBADMSG);
  return get_as<U8Array>(
    new TreeReader(a_.data).read_branch().at(0),
    SequenceType.U8_ARRAY,
  );
}

export async function rings(
  link_: Sender,
  ring: number,
  from: number,
  cnt: number,
): Promise<U8Array> {
  const a_ = await link_.send(HANDLE, RINGS, [
    3,
    (w_) => {
      w_.u32(ring);
      w_.u32(from);
      w_.u32(cnt);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(Errno.EBADMSG);
  return get_as<U8Array>(
    new TreeReader(a_.data).read_branch().at(0),
    SequenceType.U8_ARRAY,
  );
}

export async function strtab(link_: Sender): Promise<U8Array> {
  const a_ = await link_.send(HANDLE, STRTAB, null);
  if (a_.result < 0) throw new XputeError(a_.result);
  if (a_.data === null) throw new XputeError(Errno.EBADMSG);
  return get_as<U8Array>(
    new TreeReader(a_.data).read_branch().at(0),
    SequenceType.U8_ARRAY,
  );
}
