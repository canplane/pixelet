// @xpute/gpu-link/abi/gpu.ts
//
// GENERATED from gpu/wit (xpute-link-gen wit.rs)

import { type ErrorCode, of_ErrorCode } from "./gpu.enum.ts";
import {
  ERRNO_EBADMSG,
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { type Lent, load_Lent } from "@xpute/link/abi/types.ts";
import { Lift } from "@xpute/link/cabi.ts";
import {
  ComponentError,
  type Handle,
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:gpu/gpu` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x79d455;

/** `run` */
export const RUN: Method = 0;
/** `get-device` */
export const GET_DEVICE: Method = 1;
/** `[method]device.lost` */
export const DEVICE_LOST: Method = 0;
/** `[method]device.ready` */
export const DEVICE_READY: Method = 1;
/** `[method]device.settled` */
export const DEVICE_SETTLED: Method = 2;

export function load_ErrorCode(r: Lift, at: number): ErrorCode {
  const v = of_ErrorCode(r.case(at, 1));
  if (v === null) throw new XputeError(ERRNO_EBADMSG);
  return v;
}

export interface Host {
  run(records: Lent, signal_: AbortSignal): Promise<void>;
  get_device(): number;
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
          case RUN: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            const stop = new AbortController();
            implementation.run(load_Lent(r_, 0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case GET_DEVICE: {
            const v = implementation.get_device();
            return {
              result: 0,
              data: [
                4,
                (w) => {
                  w.u32(0, v);
                },
              ],
            };
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

/** Serves `implementation` under the interface's handle. */
export function mount(
  l: Pick<Link, "mount_named" | "task_return" | "on_cancel">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(l, implementation));
}

export interface HostDevice {
  lost(block: Lent, signal_: AbortSignal): Promise<void>;
  ready(signal_: AbortSignal): Promise<void>;
  settled(signal_: AbortSignal): Promise<void>;
}

/** The object over `implementation`. */
export function device(
  l: Pick<Link, "task_return" | "on_cancel">,
  implementation: HostDevice,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DEVICE_LOST: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            const stop = new AbortController();
            implementation.lost(load_Lent(r_, 0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case DEVICE_READY: {
            const stop = new AbortController();
            implementation.ready(stop.signal).then(
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
                  return l.task_return(tag, refusal_of(e));
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
          case DEVICE_SETTLED: {
            const stop = new AbortController();
            implementation.settled(stop.signal).then(
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
