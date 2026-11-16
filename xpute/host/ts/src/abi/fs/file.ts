// @xpute/host/abi/fs/file.ts
//
// GENERATED from wit/fs (xpute-link-gen wit.rs)

import type { ErrorCode } from "./types.enum.ts";
import {
  ERRNO_EINPROGRESS,
  ERRNO_ENOSYS,
} from "@xpute/kit/status/errno.spec.ts";
import { type Lent, load_Lent } from "@xpute/link/abi/types.ts";
import {
  type Callee,
  ComponentError,
  type Handle,
  type LinkObject,
  type Method,
  refusal_of,
} from "@xpute/link/link.ts";

/** The interface's handle, named by `xpute:fs/file` (frame.json5 `NAMED`). */
export const HANDLE: Handle = 0x5b6743;

/** `[method]file.size` */
export const FILE_SIZE: Method = 0;
/** `[method]file.read` */
export const FILE_READ: Method = 1;
/** `[method]file.write` */
export const FILE_WRITE: Method = 2;
/** `[method]file.set-aside` */
export const FILE_SET_ASIDE: Method = 3;
/** `[method]file.close` */
export const FILE_CLOSE: Method = 4;

export interface HostFile {
  size(signal_: AbortSignal): Promise<number>;
  read(buf: Lent, signal_: AbortSignal): Promise<number>;
  write(buf: Lent, signal_: AbortSignal): Promise<void>;
  set_aside(signal_: AbortSignal): Promise<void>;
  close(signal_: AbortSignal): Promise<void>;
  drop(): void;
}

/** The object over `implementation`. */
export function file(
  l: Pick<Callee, "task_return" | "on_cancel">,
  implementation: HostFile,
): LinkObject {
  return {
    call(m, tag, args, _ret) {
      try {
        switch (m) {
          case FILE_SIZE: {
            const stop = new AbortController();
            implementation.size(stop.signal).then(
              (x) => {
                const v = { tag: "ok", val: x } as
                  { tag: "ok"; val: number } | { tag: "err"; val: ErrorCode };
                l.task_return(tag, 0, [
                  8,
                  (w) => {
                    if (v.tag === "ok") {
                      w.u32(4, v.val);
                    } else {
                      w.u8(0, 1);
                      w.u8(4, v.val);
                    }
                  },
                ]);
              },
              (e: unknown) => {
                if (!(e instanceof ComponentError))
                  return l.task_return(tag, refusal_of(e));
                const v = { tag: "err", val: e.payload as ErrorCode } as
                  { tag: "ok"; val: number } | { tag: "err"; val: ErrorCode };
                l.task_return(tag, 0, [
                  8,
                  (w) => {
                    if (v.tag === "ok") {
                      w.u32(4, v.val);
                    } else {
                      w.u8(0, 1);
                      w.u8(4, v.val);
                    }
                  },
                ]);
              },
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case FILE_READ: {
            const stop = new AbortController();
            implementation.read(load_Lent(args, 0), stop.signal).then(
              (x) => {
                const v = { tag: "ok", val: x } as
                  { tag: "ok"; val: number } | { tag: "err"; val: ErrorCode };
                l.task_return(tag, 0, [
                  8,
                  (w) => {
                    if (v.tag === "ok") {
                      w.u32(4, v.val);
                    } else {
                      w.u8(0, 1);
                      w.u8(4, v.val);
                    }
                  },
                ]);
              },
              (e: unknown) => {
                if (!(e instanceof ComponentError))
                  return l.task_return(tag, refusal_of(e));
                const v = { tag: "err", val: e.payload as ErrorCode } as
                  { tag: "ok"; val: number } | { tag: "err"; val: ErrorCode };
                l.task_return(tag, 0, [
                  8,
                  (w) => {
                    if (v.tag === "ok") {
                      w.u32(4, v.val);
                    } else {
                      w.u8(0, 1);
                      w.u8(4, v.val);
                    }
                  },
                ]);
              },
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case FILE_WRITE: {
            const stop = new AbortController();
            implementation.write(load_Lent(args, 0), stop.signal).then(
              () => l.task_return(tag, 0, null),
              (e: unknown) => l.task_return(tag, refusal_of(e)),
            );
            l.on_cancel(tag, () => stop.abort());
            return ERRNO_EINPROGRESS;
          }
          case FILE_SET_ASIDE: {
            const stop = new AbortController();
            implementation.set_aside(stop.signal).then(
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
          case FILE_CLOSE: {
            const stop = new AbortController();
            implementation.close(stop.signal).then(
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
