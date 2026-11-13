// @pxl/client/abi/text.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Handle,
} from "@xpute/link/link.ts";
import * as types from "@xpute/link/abi/types.ts";

/** The interface's handle, named by `pxl:core/text` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x61d944;

/** `width` */
export const WIDTH: Method = 0;
/** `raster` */
export const RASTER: Method = 1;

export interface Text {
  width(weight: number, px: number, text: types.Lent): number;
  raster(
    weight: number,
    px: number,
    color: number,
    image: types.Lent,
    width: number,
    height: number,
    x: number,
    text: types.Lent,
  ): number;
}

/** The object over `imp`. */
function object(imp: Text): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case WIDTH: {
            if (args === null) return Errno.EBADMSG;
            const v = imp.width(
              args.at(0).get_number(),
              args.at(1).get_number(),
              types.read_Lent(args.at(2)),
            );
            return {
              result: 0,
              data: [
                1,
                (w) => {
                  w.f64(v);
                },
              ],
            };
          }
          case RASTER: {
            if (args === null) return Errno.EBADMSG;
            const v = imp.raster(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_number(),
              types.read_Lent(args.at(3)),
              args.at(4).get_number(),
              args.at(5).get_number(),
              args.at(6).get_number(),
              types.read_Lent(args.at(7)),
            );
            return v;
          }
          default:
            return Errno.ENOSYS;
        }
      } catch (e) {
        return refusal_of(e);
      }
    },
  };
}

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Text): void {
  l.mount_named(HANDLE, object(imp));
}
