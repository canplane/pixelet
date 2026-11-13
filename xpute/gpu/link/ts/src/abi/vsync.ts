// @xpute/gpu-link/abi/vsync.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Handle,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:gpu/vsync` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x42eda6;

/** `next` */
export const NEXT: Method = 0;

export interface Vsync {
  next(signal_: AbortSignal): Promise<void>;
}

/** The object over `imp`. */
function object(
  l: Pick<Link, "task_return" | "on_cancel">,
  imp: Vsync,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case NEXT: {
            const stop = new AbortController();
            imp.next(stop.signal).then(
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
  imp: Vsync,
): void {
  l.mount_named(HANDLE, object(l, imp));
}
