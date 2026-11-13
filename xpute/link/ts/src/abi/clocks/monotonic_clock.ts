// @xpute/link/abi/clocks/monotonic_clock.ts
//
// GENERATED from wit/clocks (xpute-link-gen wit.rs)

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
} from "../../link.ts";

/** The interface's handle, named by `xpute:clocks/monotonic-clock` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x73f703;

/** The interface's qualified name: its package and its name. */
export const NAME = uri("xpute:clocks", "monotonic-clock");

/** `now` */
export const NOW: Method = 0;
/** `wait-for` */
export const WAIT_FOR: Method = 1;

export type Mark = bigint;

export interface MonotonicClock {
  now(): bigint;
  wait_for(how_long: bigint, signal: AbortSignal): Promise<void>;
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
            return { result: 0, data: [v] };
          }
          case WAIT_FOR: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.wait_for(args.at(0).get_u64(), stop.signal).then(
              () => l.task_return(tag, 0, []),
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

export async function now(link: Sender): Promise<bigint> {
  const a = await link.send(HANDLE, NOW, []);
  if (a.result < 0) throw new XputeError(a.result);
  if (a.data === null) throw new XputeError(Errno.EBADMSG);
  return new TreeReader(a.data).read_branch().at(0).get_u64();
}

export async function wait_for(link: Sender, how_long: bigint): Promise<void> {
  const a = await link.send(HANDLE, WAIT_FOR, [how_long]);
  if (a.result < 0) throw new XputeError(a.result);
}
