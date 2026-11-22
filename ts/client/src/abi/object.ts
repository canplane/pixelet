// @pxl/client/abi/object.ts
//
// GENERATED from spec/client/core/wit (xpute-link-gen wit.rs)

import type { ErrorCode } from "@xpute/host/abi/fs/types.enum.ts";
import {
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { type Lent, load_Lent } from "@xpute/link/abi/types.ts";
import {
  type Callee,
  ComponentError,
  type Handle,
  type LinkObject,
  type Method,
  err_of,
  store_err,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:core/object` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x4edab0;

/** `apply` */
export const APPLY: Method = 0;

export interface Host {
  apply(batch: Lent, signal_: AbortSignal): Promise<void>;
}

/** The object over `implementation`. */
function object(
  l: Pick<Callee, "task_return" | "task_err" | "on_cancel">,
  implementation: Host,
): LinkObject {
  return {
    call(m, tag, args, ret) {
      try {
        switch (m) {
          case APPLY: {
            const stop = new AbortController();
            implementation.apply(load_Lent(args, 0), stop.signal).then(
              () => {
                const v = { tag: "ok" } as
                  { tag: "ok" } | { tag: "err"; val: ErrorCode };
                l.task_return(tag, 0, [
                  2,
                  (w) => {
                    if (v.tag === "ok") {
                    } else {
                      w.u8(0, 1);
                      w.u8(1, v.val);
                    }
                  },
                ]);
              },
              (e: unknown) => {
                if (!(e instanceof ComponentError))
                  return l.task_err(tag, err_of(e));
                const v = { tag: "err", val: e.payload as ErrorCode } as
                  { tag: "ok" } | { tag: "err"; val: ErrorCode };
                l.task_return(tag, 0, [
                  2,
                  (w) => {
                    if (v.tag === "ok") {
                    } else {
                      w.u8(0, 1);
                      w.u8(1, v.val);
                    }
                  },
                ]);
              },
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          default:
            throw new XputeError(ERRNO_ENOSYS);
        }
      } catch (e) {
        return store_err(ret, err_of(e));
      }
    },
    drop() {},
  };
}

/** Mounts `implementation` under the interface's handle. */
export function mount(
  l: Pick<Callee, "mount_named" | "task_return" | "task_err" | "on_cancel">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(l, implementation));
}
