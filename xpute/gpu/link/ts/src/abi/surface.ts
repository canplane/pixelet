// @xpute/gpu-link/abi/surface.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import { XputeError } from "@xpute/kit/status/error.ts";
import { type Method, type Ref } from "@xpute/link/link.ts";

/** `[method]surface.resize` */
export const SURFACE_RESIZE: Method = 0;

export async function surface_resize(
  ref_: Ref,
  width: number,
  height: number,
  dpr: number,
): Promise<void> {
  const a_ = await ref_.link.send(ref_.handle, SURFACE_RESIZE, [
    3,
    (w_) => {
      w_.u32(width);
      w_.u32(height);
      w_.f64(dpr);
    },
  ]);
  if (a_.result < 0) throw new XputeError(a_.result);
}
