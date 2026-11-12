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

/** `[method]file.read` */
export const FILE_READ: Method = 0;
/** `[method]file.write` */
export const FILE_WRITE: Method = 1;
/** `[method]file.changed` */
export const FILE_CHANGED: Method = 2;
/** `[method]file.close` */
export const FILE_CLOSE: Method = 3;

export interface File {
  read(at: number, len: number): Promise<number>;
  write(at: number, len: number): Promise<void>;
  changed(): Promise<void>;
  close(): Promise<void>;
}

/** The object over `imp`. */
export function file(link: Pick<Link, "complete">, imp: File): LinkObject {
  return {
    call(m, tag, args) {
      try {
        switch (m) {
          case FILE_READ: {
            if (args === null) return Errno.EBADMSG;
            imp.read(args.at(0).get_number(), args.at(1).get_number()).then(
              (v) => link.complete(tag, v, []),
              (e: unknown) => link.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case FILE_WRITE: {
            if (args === null) return Errno.EBADMSG;
            imp.write(args.at(0).get_number(), args.at(1).get_number()).then(
              () => link.complete(tag, 0, []),
              (e: unknown) => link.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case FILE_CHANGED: {
            imp.changed().then(
              () => link.complete(tag, 0, []),
              (e: unknown) => link.complete(tag, refusal_of(e)),
            );
            return Errno.EINPROGRESS;
          }
          case FILE_CLOSE: {
            imp.close().then(
              () => link.complete(tag, 0, []),
              (e: unknown) => link.complete(tag, refusal_of(e)),
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

export async function file_read(
  ref: Ref,
  at: number,
  len: number,
): Promise<number> {
  const a = await ref.link.send(ref.handle, FILE_READ, [at, len]);
  if (a.result < 0) throw new XputeError(a.result);
  return a.result;
}

export async function file_write(
  ref: Ref,
  at: number,
  len: number,
): Promise<void> {
  const a = await ref.link.send(ref.handle, FILE_WRITE, [at, len]);
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
