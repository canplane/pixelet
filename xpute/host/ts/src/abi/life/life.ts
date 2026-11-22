// @xpute/host/abi/life/life.ts
//
// GENERATED from wit/life (xpute-link-gen wit.rs)

import {
  type Handle,
  type Method,
  type Sender,
  answer,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:life/life` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x46edb9;

/** `hidden` */
export const HIDDEN: Method = 0;
/** `shown` */
export const SHOWN: Method = 1;
/** `attached` */
export const ATTACHED: Method = 2;

export async function hidden(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, HIDDEN, null);
  answer(a_);
}

export async function shown(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, SHOWN, null);
  answer(a_);
}

export async function attached(link_: Sender): Promise<void> {
  const a_ = await link_.send(HANDLE, ATTACHED, null);
  answer(a_);
}
