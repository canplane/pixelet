// @xpute/web-host/abi/stop/stop.ts
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

import type { u64 } from "@xpute/kit/abi/word.ts";
import { ERRNO_EBADMSG, ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import { Lift } from "@xpute/link/cabi.ts";
import {
  type Handle,
  type LinkObject,
  type Method,
  type Side,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:stop/stop` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x4eccaa;

/** `stopped` */
export const STOPPED: Method = 0;

export interface Host {
  stopped(
    errnum: number,
    file: string,
    line: number,
    n1: u64,
    n2: u64,
    message: string | null,
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
              r_.case(32, 2) === 0 ? null : r_.str(36),
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
