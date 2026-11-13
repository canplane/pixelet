// @xpute/link/abi/fs/file.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
} from "../../link.ts";
import * as types from "../types.ts";

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

export interface File {
  size(signal_: AbortSignal): Promise<number>;
  read(buf: types.Lent, signal_: AbortSignal): Promise<number>;
  write(buf: types.Lent, signal_: AbortSignal): Promise<void>;
  changed(signal_: AbortSignal): Promise<void>;
  close(signal_: AbortSignal): Promise<void>;
}

/** The object over `imp`. */
export function file(
  l: Pick<Link, "task_return" | "on_cancel">,
  imp: File,
): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case FILE_SIZE: {
            const stop = new AbortController();
            imp.size(stop.signal).then(
              (v) => l.task_return(tag, v),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_READ: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.read(types.read_Lent(args.at(0)), stop.signal).then(
              (v) => l.task_return(tag, v),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_WRITE: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.write(types.read_Lent(args.at(0)), stop.signal).then(
              () => l.task_return(tag, 0),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_CHANGED: {
            const stop = new AbortController();
            imp.changed(stop.signal).then(
              () => l.task_return(tag, 0),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_CLOSE: {
            const stop = new AbortController();
            imp.close(stop.signal).then(
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
