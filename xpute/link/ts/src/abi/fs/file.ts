// @xpute/link/abi/fs/file.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Ref,
  uri,
} from "../../link.ts";
import * as types from "../types.ts";

/** The interface's qualified name: its package and its name. */
export const NAME = uri("xpute:fs", "file");

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
  size(signal: AbortSignal): Promise<number>;
  read(buf: types.Lent, signal: AbortSignal): Promise<number>;
  write(buf: types.Lent, signal: AbortSignal): Promise<void>;
  changed(signal: AbortSignal): Promise<void>;
  close(signal: AbortSignal): Promise<void>;
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
              (v) => l.task_return(tag, v, []),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_READ: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.read(types.read_Lent(args.at(0)), stop.signal).then(
              (v) => l.task_return(tag, v, []),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_WRITE: {
            if (args === null) return Errno.EBADMSG;
            const stop = new AbortController();
            imp.write(types.read_Lent(args.at(0)), stop.signal).then(
              () => l.task_return(tag, 0, []),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_CHANGED: {
            const stop = new AbortController();
            imp.changed(stop.signal).then(
              () => l.task_return(tag, 0, []),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return Errno.EINPROGRESS;
          }
          case FILE_CLOSE: {
            const stop = new AbortController();
            imp.close(stop.signal).then(
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

export async function file_size(ref: Ref): Promise<number> {
  const a = await ref.link.send(ref.handle, FILE_SIZE, []);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

export async function file_read(ref: Ref, buf: types.Lent): Promise<number> {
  const a = await ref.link.send(ref.handle, FILE_READ, [types.write_Lent(buf)]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

export async function file_write(ref: Ref, buf: types.Lent): Promise<void> {
  const a = await ref.link.send(ref.handle, FILE_WRITE, [
    types.write_Lent(buf),
  ]);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function file_changed(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, FILE_CHANGED, []);
  if (a.result < 0) throw new XputeError(a.result);
}

export async function file_close(ref: Ref): Promise<void> {
  const a = await ref.link.send(ref.handle, FILE_CLOSE, []);
  if (a.result < 0) throw new XputeError(a.result);
}
