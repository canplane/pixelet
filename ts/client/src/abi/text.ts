// @pxl/client/abi/text.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { type Lent, load_Lent } from "@xpute/link/abi/types.ts";
import { Lift } from "@xpute/link/cabi.ts";
import {
  type Handle,
  type LinkObject,
  type Method,
  type Side,
  refusal_of,
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
    call(m, tag, args) {
      try {
        switch (m) {
          case WIDTH: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            const v = implementation.width(
              r_.u32(0),
              r_.f64(8),
              load_Lent(r_, 16),
            );
            return {
              result: 0,
              data: [
                8,
                (w) => {
                  w.f64(0, v);
                },
              ],
            };
          }
          case RASTER: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            implementation.raster(
              r_.u32(0),
              r_.f64(8),
              r_.u32(16),
              load_Lent(r_, 20),
              r_.u32(28),
              r_.u32(32),
              r_.f64(40),
              load_Lent(r_, 48),
            );
            return 0;
          }
          default:
            return ERRNO_ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
    drop() {},
  };
}

/** Serves `implementation` under the interface's handle. */
export function mount(
  l: Pick<Side, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
