// @pxl/client/abi/setting.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { load_ErrorCode } from "./types.ts";
import { ERRNO_EBADMSG } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Lift } from "@xpute/link/cabi.ts";
import {
  ComponentError,
  type Handle,
  type Method,
  type Sender,
  answer,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/setting` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x6f63cb;

/** `set` */
export const SET: Method = 0;

export async function set(
  link_: Sender,
  key: number,
  val: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, SET, [
    8,
    (w_) => {
      w_.u32(0, key);
      w_.u32(4, val);
    },
  ]);
  answer(a_);
  if (a_.data === null) throw new XputeError(ERRNO_EBADMSG);
  const r_ = new Lift(a_.data);
  const v_ =
    r_.case(0, 2) === 0
      ? { tag: "ok" as const }
      : { tag: "err" as const, val: load_ErrorCode(r_, 1) };
  if (v_.tag === "err") throw new ComponentError(v_.val);
  return;
}
