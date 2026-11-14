// @pxl/client/abi/source.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { type Declaration, store_Declaration } from "./types.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Handle, Method, Sender } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/source` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x6a88bf;

/** `declare` */
export const DECLARE: Method = 0;

export async function declare(link_: Sender, decl: Declaration): Promise<void> {
  const a_ = await link_.send(HANDLE, DECLARE, [
    28,
    (w_) => {
      store_Declaration(w_, 0, decl);
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
