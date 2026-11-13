// @xpute/link/abi/life/life.ts
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Sender, type Handle } from "../../link.ts";

/** The interface's handle, named by `xpute:life/life` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x46edb9;

/** `hidden` */
export const HIDDEN: Method = 0;
/** `shown` */
export const SHOWN: Method = 1;

export async function hidden(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, HIDDEN, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}

export async function shown(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, SHOWN, null);
  if (a_.result < 0) throw new XputeError(a_.result);
}
