// @pxl/client/abi/text.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { TreeReader } from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "@xpute/link/link.ts";
import * as types from "@xpute/link/abi/types.ts";

/** The interface's handle, named by `pxl:core/text` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x61d944;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "text");

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
            return { result: 0, data: [v] };
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

export async function width(
  link: Sender,
  weight: number,
  px: number,
  text: types.Lent,
): Promise<number> {
  const a = await link.send(HANDLE, WIDTH, [
    weight,
    px,
    types.write_Lent(text),
  ]);
  if (a.result < 0) throw new XputeError(a.result);
  if (a.data === null) throw new XputeError(Errno.EBADMSG);
  return new TreeReader(a.data).read_branch().at(0).get_number();
}

export async function raster(
  link: Sender,
  weight: number,
  px: number,
  color: number,
  image: types.Lent,
  width: number,
  height: number,
  x: number,
  text: types.Lent,
): Promise<number> {
  const a = await link.send(HANDLE, RASTER, [
    weight,
    px,
    color,
    types.write_Lent(image),
    width,
    height,
    x,
    types.write_Lent(text),
  ]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}
