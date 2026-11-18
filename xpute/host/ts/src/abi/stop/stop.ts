// @xpute/host/abi/stop/stop.ts
//
// GENERATED from wit/stop (xpute-link-gen wit.rs)

import type { u64 } from "@xpute/kit/abi/word.ts";
import { ERRNO_ENOSYS } from "@xpute/kit/status/errno.spec.ts";
import {
  type Callee,
  type Handle,
  type LinkObject,
  type Method,
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
    msg: string | null,
  ): void;
}

/** The object over `implementation`. */
function object(implementation: Host): LinkObject {
  return {
    call(m, _tag, args, _ret) {
      try {
        switch (m) {
          case STOPPED: {
            implementation.stopped(
              args.i32(0),
              args.str(4),
              args.u32(12),
              args.u64(16),
              args.u64(24),
              args.case(32, 2) === 0 ? null : args.str(36),
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

/** Mounts `implementation` under the interface's handle. */
export function mount(
  l: Pick<Callee, "mount_named">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(implementation));
}
