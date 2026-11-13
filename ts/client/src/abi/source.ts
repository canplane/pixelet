// @pxl/client/abi/source.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { U32Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { Branch } from "@xpute/kit/wire/xtp/mod.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Sender,
  type Handle,
  uri,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/source` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x6a88bf;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("pxl:core", "source");

/** `declare` */
export const DECLARE: Method = 0;

export interface Source {
  declare(min_zoom: number, format: U32Array, shard: string[]): void;
}

/** The object over `imp`. */
function object(imp: Source): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DECLARE: {
            if (args === null) return Errno.EBADMSG;
            imp.declare(
              args.at(0).get_number(),
              args.at(1).get() as U32Array,
              args
                .at(2)
                .as_branch()
                .children()
                .map((c) => c.get_str()),
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

/** Serves `imp` under the interface's handle. */
export function mount(l: Pick<Link, "mount_named">, imp: Source): void {
  l.mount_named(HANDLE, object(imp));
}

export async function declare(
  link: Sender,
  min_zoom: number,
  format: U32Array,
  shard: string[],
): Promise<void> {
  const a = await link.send(HANDLE, DECLARE, [
    min_zoom,
    format,
    new Branch(shard.map((x) => x)),
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}
