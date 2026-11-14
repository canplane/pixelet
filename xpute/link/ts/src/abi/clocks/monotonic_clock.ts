// @xpute/link/abi/clocks/monotonic_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import { Lift } from "../../cabi.ts";
import {
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "../../link.ts";
import {
  ERRNO_EBADMSG,
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";

/** The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x73f703;

/** `now` */
export const NOW: Method = 0;
/** `wait-for` */
export const WAIT_FOR: Method = 1;

export type Mark = bigint;

export interface Host {
  now(): bigint;
  wait_for(how_long: bigint, signal_: AbortSignal): Promise<void>;
}

/** The object over `imp`. */
function object(
  l: Pick<Link, "task_return" | "on_cancel">,
  imp: Host,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case NOW: {
            const v = imp.now();
            return {
              result: 0,
              data: [
                8,
                (w) => {
                  w.u64(0, v);
                },
              ],
            };
          }
          case WAIT_FOR: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            const stop = new AbortController();
            imp.wait_for(r_.u64(0), stop.signal).then(
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
  };
}

/** Serves `imp` under the interface's handle. */
export function mount(
  l: Pick<Link, "mount_named" | "task_return" | "on_cancel">,
  imp: Host,
): void {
  l.mount_named(HANDLE, object(l, imp));
}
