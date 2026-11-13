// @pxl/client/abi/setting.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Sender, type Handle } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/setting` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x6f63cb;

/** `dev` */
export const DEV: Method = 0;
/** `set` */
export const SET: Method = 1;

export async function dev(link_: Sender, dev: boolean): Promise<void> {
  const a_ = await link_.send(HANDLE, DEV, [
    1,
    (w_) => {
      w_.bool(dev);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function set(
  link_: Sender,
  key: number,
  val: number,
): Promise<void> {
  const a_ = await link_.send(HANDLE, SET, [
    2,
    (w_) => {
      w_.u32(key);
      w_.u32(val);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}
