// @xpute/link/abi/stop/stop.ts
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

import { Lift } from "../../cabi.ts";
import {
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "../../link.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";

/** The interface's handle, named by `xpute:stop/stop` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x4eccaa;

/** `stopped` */
export const STOPPED: Method = 0;

export interface Host {
  stopped(
    errnum: number,
    file: string,
    line: number,
    n1: bigint,
    n2: bigint,
  ): void;
}

/** The object over `implementation`. */
function object(implementation: Host): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case STOPPED: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            implementation.stopped(
              r_.i32(0),
              r_.str(4),
              r_.u32(12),
              r_.u64(16),
              r_.u64(24),
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
  };
}

/** Serves `implementation` under the interface's handle. */
export function mount(
  l: Pick<Link, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
