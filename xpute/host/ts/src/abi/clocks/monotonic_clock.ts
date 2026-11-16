// @xpute/host/abi/clocks/monotonic_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import type { u64 } from "@xpute/kit/abi/word.ts";
import {
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import {
  type Callee,
  type Handle,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x73f703;

/** `now` */
export const NOW: Method = 0;
/** `wait-for` */
export const WAIT_FOR: Method = 1;

export type Mark = u64;

export interface Host {
  now(): u64;
  wait_for(how_long: u64, signal_: AbortSignal): Promise<void>;
}

/** The object over `implementation`. */
function object(
  l: Pick<Callee, "task_return" | "on_cancel">,
  implementation: Host,
): LinkObject {
  return {
    call(m, tag, args, ret) {
      try {
        switch (m) {
          case NOW: {
            const v = implementation.now();
            ret.start(8);
            ret.u64(0, v);
            return 0;
          }
          case WAIT_FOR: {
            const stop = new AbortController();
            implementation.wait_for(args.u64(0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
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
  l: Pick<Callee, "mount_named" | "task_return" | "on_cancel">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(l, implementation));
}
