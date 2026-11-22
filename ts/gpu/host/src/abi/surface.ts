// @pxl/gpu/host/src/abi/surface.ts
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

import {
  type Handle,
  type Method,
  type Ref,
  answer,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:gpu/surface` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x76d49a;

/** `[method]surface.resize` */
export const SURFACE_RESIZE: Method = 0;

export async function surface_resize(
  ref_: Ref,
  width: number,
  height: number,
  dpr: number,
): Promise<void> {
  const a_ = await ref_.link.send(ref_.handle, SURFACE_RESIZE, [
    16,
    (w_) => {
      w_.u32(0, width);
      w_.u32(4, height);
      w_.f64(8, dpr);
    },
  ]);
  answer(a_);
}
