// @pxl/gpu/host/src/abi/gpu.ts
//
// GENERATED from spec/gpu/wit (xpute-link-gen wit.rs)

import { type ErrorCode, of_ErrorCode } from "./gpu.enum.ts";
import {
  ERRNO_EBADMSG,
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import { type Lent, load_Lent } from "@xpute/link/abi/types.ts";
import type { Lift } from "@xpute/link/cabi.ts";
import {
  type Callee,
  ComponentError,
  type Handle,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `pxl:gpu/gpu` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x54bd98;

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
  run(recs: Lent, signal_: AbortSignal): Promise<void>;
  get_device(): number;
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
          case RUN: {
            const stop = new AbortController();
            implementation.run(load_Lent(args, 0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case GET_DEVICE: {
            const v = implementation.get_device();
            ret.start(4);
            ret.u32(0, v);
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
  l: Pick<Callee, "mount_named" | "task_return" | "on_cancel">,
  implementation: Host,
): void {
  l.mount_named(HANDLE, object(l, implementation));
}

export interface HostDevice {
  lost(block: Lent, signal_: AbortSignal): Promise<void>;
  ready(signal_: AbortSignal): Promise<void>;
  settled(signal_: AbortSignal): Promise<void>;
  drop(): void;
}

/** The object over `implementation`. */
export function device(
  l: Pick<Callee, "task_return" | "on_cancel">,
  implementation: HostDevice,
): LinkObject {
  return {
    call(m, tag, args, _ret) {
      try {
        switch (m) {
          case DEVICE_LOST: {
            const stop = new AbortController();
            implementation.lost(load_Lent(args, 0), stop.signal).then(
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
    drop() {
      implementation.drop();
    },
  };
}
