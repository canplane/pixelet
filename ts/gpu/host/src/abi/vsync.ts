// @pxl/gpu/host/src/abi/vsync.ts
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

import type { u64 } from "@xpute/kit/abi/word.ts";
import {
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import type { Lift, Lower } from "@xpute/link/cabi.ts";
import {
  type Callee,
  type Handle,
  type LinkObject,
  type Method,
  err_of,
  store_err,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:gpu/vsync` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x51cb2a;

/** `next` */
export const NEXT: Method = 0;

export interface Frame {
  at: u64;
  gap: u64;
}

export const FRAME_SZ = 16;
export const FRAME_ALIGN = 8;

export function store_Frame(w: Lower, at: number, v: Frame): void {
  w.u64(at, v.at);
  w.u64(at + 8, v.gap);
}

export function load_Frame(r: Lift, at: number): Frame {
  return {
    at: r.u64(at),
    gap: r.u64(at + 8),
  };
}

export interface Host {
  next(after: u64, signal_: AbortSignal): Promise<Frame>;
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
          case NEXT: {
            const stop = new AbortController();
            implementation.next(args.u64(0), stop.signal).then(
              (v) =>
                l.task_return(tag, 0, [
                  FRAME_SZ,
                  (w) => {
                    store_Frame(w, 0, v);
                  },
                ]),
              (e: unknown) => l.task_err(tag, err_of(e)),
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
