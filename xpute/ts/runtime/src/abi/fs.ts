// @xpute/runtime/abi/fs.ts
//
// GENERATED from wit (xpute-gen spec/wit.rs)

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";
import {
  type Link,
  type LinkObject,
  type Method,
  refusal_of,
  type Ref,
} from "../link.ts";
import * as link from "./link.ts";

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
  size(): Promise<number>;
  read(buf: link.Lent): Promise<number>;
  write(buf: link.Lent): Promise<void>;
  changed(): Promise<void>;
  close(): Promise<void>;
}

/** The object over `imp`. */
export function file(l: Pick<Link, "complete">, imp: File): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case FILE_SIZE: {
            imp.size().then(
              (v) => l.complete(tag, v, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case FILE_READ: {
            if (args === null) return Errno.EBADMSG;
            imp.read(link.read_Lent(args.at(0))).then(
              (v) => l.complete(tag, v, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case FILE_WRITE: {
            if (args === null) return Errno.EBADMSG;
            imp.write(link.read_Lent(args.at(0))).then(
              () => l.complete(tag, 0, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case FILE_CHANGED: {
            imp.changed().then(
              () => l.complete(tag, 0, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case FILE_CLOSE: {
            imp.close().then(
              () => l.complete(tag, 0, []),
              (e: unknown) => l.complete(tag, refusal_of(e)),
            );
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

export async function file_read(ref: Ref, buf: link.Lent): Promise<number> {
  const a = await ref.link.send(ref.handle, FILE_READ, [link.write_Lent(buf)]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

export async function file_write(ref: Ref, buf: link.Lent): Promise<void> {
  const a = await ref.link.send(ref.handle, FILE_WRITE, [link.write_Lent(buf)]);
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
