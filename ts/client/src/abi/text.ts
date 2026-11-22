// @pxl/client/abi/text.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { type Lent, load_Lent } from "@xpute/link/abi/types.ts";
import {
  type Callee,
  type Handle,
  type LinkObject,
  type Method,
  err_of,
  store_err,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/text` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x61d944;

/** `width` */
export const WIDTH: Method = 0;
/** `raster` */
export const RASTER: Method = 1;

export interface Host {
  width(weight: number, px: number, text: Lent): number;
  raster(
    weight: number,
    px: number,
    color: number,
    image: Lent,
    width: number,
    height: number,
    x: number,
    text: Lent,
  ): void;
}

/** The object over `implementation`. */
function object(implementation: Host): LinkObject {
  return {
    call(m, _tag, args, ret) {
      try {
        switch (m) {
          case WIDTH: {
            const v = implementation.width(
              args.u32(0),
              args.f64(8),
              load_Lent(args, 16),
            );
            ret.start(8);
            ret.f64(0, v);
            return 0;
          }
          case RASTER: {
            implementation.raster(
              args.u32(0),
              args.f64(8),
              args.u32(16),
              load_Lent(args, 24),
              args.u32(40),
              args.u32(44),
              args.f64(48),
              load_Lent(args, 56),
            );
            return 0;
          }
          default:
            throw new XputeError(ERRNO_ENOSYS);
        }
      } catch (e) {
        return store_err(ret, err_of(e));
      }
    },
    drop() {},
  };
}

/** Mounts `implementation` under the interface's handle. */
export function mount(
  l: Pick<Callee, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
