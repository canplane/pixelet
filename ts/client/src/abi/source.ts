// @pxl/client/abi/source.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { U32Array } from "@xpute/kit/abi/array.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { SequenceType } from "@xpute/kit/wire/xtp/mod.ts";
import { type Method, type Sender, type Handle } from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/source` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x6a88bf;

/** `declare` */
export const DECLARE: Method = 0;

export async function declare(
  link_: Sender,
  min_zoom: number,
  format: U32Array,
  shard: string[],
): Promise<void> {
  const a_ = await link_.send(HANDLE, DECLARE, [
    3,
    (w_) => {
      w_.u32(min_zoom);
      w_.array(SequenceType.U32_ARRAY, format);
      w_.branch(shard.length, (w_) =>
        shard.forEach((x) => {
          w_.str(x);
        }),
      );
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}
