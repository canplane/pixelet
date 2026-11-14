// @xpute/gpu-link/abi/vsync.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import {
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import {
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:gpu/vsync` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x42eda6;

/** `next` */
export const NEXT: Method = 0;

export interface Host {
  next(signal_: AbortSignal): Promise<void>;
}

/** The object over `implementation`. */
function object(
  l: Pick<Link, "task_return" | "on_cancel">,
  implementation: Host,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case NEXT: {
            const stop = new AbortController();
            implementation.next(stop.signal).then(
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

/** Serves `implementation` under the interface's handle. */
export function mount(
  l: Pick<Link, "mount_named" | "task_return" | "on_cancel">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(l, implementation));
}
