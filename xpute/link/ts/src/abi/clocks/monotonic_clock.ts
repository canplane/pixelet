// @xpute/link/abi/clocks/monotonic_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Handle,
} from "../../link.ts";

/** The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x73f703;

/** `now` */
export const NOW: Method = 0;
/** `wait-for` */
export const WAIT_FOR: Method = 1;

export type Mark = bigint;

export interface MonotonicClock {
  now(): bigint;
  wait_for(how_long: bigint, signal_: AbortSignal): Promise<void>;
}

/** The object over `imp`. */
function object(
  l: Pick<Link, "task_return" | "on_cancel">,
  imp: MonotonicClock,
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
                1,
                (w) => {
                  w.u64(v);
                },
              ],
            };
          }
          case WAIT_FOR: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.wait_for(args.at(0).get_u64(), stop.signal).then(
              () => l.task_return(tag, 0),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
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
export function mount(
  l: Pick<Link, "mount_named" | "task_return" | "on_cancel">,
  imp: MonotonicClock,
): void {
  l.mount_named(HANDLE, object(l, imp));
}
