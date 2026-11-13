// @xpute/gpu-link/abi/surface.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type LinkObject,
  type Method,
  refusal_of,
  type Ref,
  uri,
} from "@xpute/link/link.ts";

/** The interface's qualified name: its package and its name. */
export const NAME = uri("xpute:gpu", "surface");

/** `[method]surface.resize` */
export const SURFACE_RESIZE: Method = 0;

export interface Surface {
  resize(width: number, height: number, dpr: number): void;
}

/** The object over `imp`. */
export function surface(imp: Surface): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case SURFACE_RESIZE: {
            if (args === null) return Errno.EBADMSG;
            imp.resize(
              args.at(0).get_number(),
              args.at(1).get_number(),
              args.at(2).get_number(),
            );
            return 0;
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

export async function surface_resize(
  ref: Ref,
  width: number,
  height: number,
  dpr: number,
): Promise<void> {
  const a = await ref.link.send(ref.handle, SURFACE_RESIZE, [
    width,
    height,
    dpr,
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}
