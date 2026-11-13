// @xpute/gpu-link/abi/gpu.ts
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
import * as types from "@xpute/link/abi/types.ts";

/** The interface's handle, named by `xpute:gpu/gpu` (frame.json `NAMED`). */
export const HANDLE: Handle = 0x79d455;

/** `run` */
export const RUN: Method = 0;
/** `[method]device.lost` */
export const DEVICE_LOST: Method = 0;
/** `[method]device.ready` */
export const DEVICE_READY: Method = 1;
/** `[method]device.settled` */
export const DEVICE_SETTLED: Method = 2;

export interface Gpu {
  run(records: types.Lent, signal_: AbortSignal): Promise<void>;
}

/** The object over `imp`. */
function object(
  l: Pick<Link, "task_return" | "on_cancel">,
  imp: Gpu,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case RUN: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.run(types.read_Lent(args.at(0)), stop.signal).then(
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
  imp: Gpu,
): void {
  l.mount_named(HANDLE, object(l, imp));
}

export interface Device {
  lost(block: types.Lent, signal_: AbortSignal): Promise<void>;
  ready(signal_: AbortSignal): Promise<number>;
  settled(signal_: AbortSignal): Promise<void>;
}

/** The object over `imp`. */
export function device(
  l: Pick<Link, "task_return" | "on_cancel">,
  imp: Device,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case DEVICE_LOST: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.lost(types.read_Lent(args.at(0)), stop.signal).then(
              () => l.task_return(tag, 0),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case DEVICE_READY: {
            const stop = new AbortController();
            imp.ready(stop.signal).then(
              (v) => l.task_return(tag, v),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case DEVICE_SETTLED: {
            const stop = new AbortController();
            imp.settled(stop.signal).then(
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
