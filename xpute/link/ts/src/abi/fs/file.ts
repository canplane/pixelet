// @xpute/link/abi/fs/file.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import { Lift } from "../../cabi.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "../../link.ts";
import { type Lent, load_Lent } from "../types.ts";
import {
  ERRNO_EBADMSG,
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";

/** `[method]file.size` */
export const FILE_SIZE: Method = 0;
/** `[method]file.read` */
export const FILE_READ: Method = 1;
/** `[method]file.write` */
export const FILE_WRITE: Method = 2;
/** `[method]file.changed` */
export const FILE_CHANGED: Method = 3;
/** `[method]file.close` */
export const FILE_CLOSE: Method = 4;

export interface HostFile {
  size(signal_: AbortSignal): Promise<number>;
  read(buf: Lent, signal_: AbortSignal): Promise<number>;
  write(buf: Lent, signal_: AbortSignal): Promise<void>;
  changed(signal_: AbortSignal): Promise<void>;
  close(signal_: AbortSignal): Promise<void>;
}

/** The object over `imp`. */
export function file(
  l: Pick<Link, "task_return" | "on_cancel">,
  imp: HostFile,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case FILE_SIZE: {
            const stop = new AbortController();
            imp.size(stop.signal).then(
              (v) =>
                l.task_return(tag, 0, [
                  4,
                  (w) => {
                    w.u32(0, v);
                  },
                ]),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case FILE_READ: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            const stop = new AbortController();
            imp.read(load_Lent(r_, 0), stop.signal).then(
              (v) =>
                l.task_return(tag, 0, [
                  4,
                  (w) => {
                    w.u32(0, v);
                  },
                ]),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case FILE_WRITE: {
            if (args === null) return ERRNO_EBADMSG;
            const r_ = new Lift(args);
            const stop = new AbortController();
            imp.write(load_Lent(r_, 0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case FILE_CHANGED: {
            const stop = new AbortController();
            imp.changed(stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case FILE_CLOSE: {
            const stop = new AbortController();
            imp.close(stop.signal).then(
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
