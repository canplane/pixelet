// @xpute/link/abi/stop/stop.ts
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Handle,
} from "../../link.ts";

/** The interface's handle, named by `xpute:stop/stop` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x4eccaa;

/** `stopped` */
export const STOPPED: Method = 0;

export interface Stop {
  stopped(
    errnum: number,
    file: string,
    line: number,
    n1: bigint,
    n2: bigint,
  ): void;
}

/** The object over `imp`. */
function object(imp: Stop): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case STOPPED: {
            if (args === null) return Errno.EBADMSG;
            imp.stopped(
              args.at(0).get_number(),
              args.at(1).get_str(),
              args.at(2).get_number(),
              args.at(3).get_u64(),
              args.at(4).get_u64(),
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
export function mount(l: Pick<Link, "mount_named">, imp: Stop): void {
  l.mount_named(HANDLE, object(imp));
}
