// @pxl/client/abi/source.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import type { Handle, Method, Sender } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/source` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x6a88bf;

/** `declare` */
export const DECLARE: Method = 0;

export async function declare(
  link_: Sender,
  min_zoom: number,
  formats: Uint32Array<ArrayBuffer>,
  shards: string[],
): Promise<void> {
  const a_ = await link_.send(HANDLE, DECLARE, [
    20,
    (w_) => {
      w_.u32(0, min_zoom);
      w_.arr(4, formats);
      {
        const b0 = w_.list(12, shards.length, 8, 4);
        if (b0 !== null)
          shards.forEach((x0, i0) => {
            w_.str(b0 + i0 * 8, x0);
          });
      }
    },
  ]);
  if (a_.result !== 0) throw new XputeError(a_.result);
}
