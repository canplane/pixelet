// @xpute/host/ipc/sys.ts

/**
 * The host's end of a guest's system calls (rust/guest/src/ipc/sys.rs):
 * the records a turn left, run after it returns and before the next begins, so
 * whatever they name in guest memory is written before the guest reads it.
 */

import type { U32Array, U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { each_record, text_of } from "./stream.ts";
import { flag, HEAD, op } from "./sys.spec.ts";

/** Why an open did not answer with a file: an errno, and whatever number the
 * host refused with (an HTTP status) for a reader to print. */
export class FileError extends Error {
  constructor(readonly errno: Errno, readonly detail: u32 = 0) {
    super(`errno ${errno}`);
  }
}

interface Abort {
  errno: number;
  file: string;
  line: u32;
  column: u32;
  a: bigint;
  b: bigint;
}

export interface SysHost {
  /** The file at `path` below `root`, whole, or a FileError; `signal` is
   * aborted when the guest closes it first. */
  open(root: u32, path: string, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit(root: u32, path: string, bytes: U8Array): void;
  opened(fd: u32, res: i32, detail: u32): void;
  abort(report: Abort): void;
  /** A call of the program's own: its number, the record's words, and where
   * its arguments begin in them. */
  program(code: u32, w: U32Array, a: u32): void;
}

interface File {
  root: u32;
  path: string;
  controller: AbortController;
  bytes: U8Array | null;
  written: U8Array[] | null;
}

/** No name empty, `.` or `..`, so a path never leaves its root. */
function walks_down(path: string): boolean {
  return path.split("/").every((name) => name !== "" && name !== "." && name !== "..");
}

function open_file(files: Sys, host: SysHost, fd: u32, root: u32, flags: u32, path: string): void {
  const file: File = { root, path, controller: new AbortController(), bytes: null, written: null };
  files.open.set(fd, file);
  // An answer is a submission, and a full ring rings a turn that would
  // overwrite the records this run is still reading, so answers wait until it ends.
  const answer = (res: i32, detail: u32) => {
    if (files.open.get(fd) !== file) return;
    if (res < 0) files.open.delete(fd);
    host.opened(fd, res, detail);
  };
  const failed = (err: unknown) => {
    if (!(err instanceof FileError)) console.warn(`[xpute/sys] open ${root}/${path}: ${err instanceof Error ? err.message : String(err)}`);
    const [errno, detail] = err instanceof FileError ? [err.errno, err.detail] : [Errno.EIO, 0];
    answer(errno, detail);
  };
  const read = (bytes: U8Array) => {
    if (files.open.get(fd) !== file) return;
    file.bytes = bytes;
    answer(bytes.byteLength, 0);
  };
  if (!walks_down(path)) return queueMicrotask(() => failed(new FileError(Errno.EACCES)));
  if (flags === flag.O_WRONLY) {
    file.written = [];
    return queueMicrotask(() => answer(0, 0));
  }
  let got: U8Array | Promise<U8Array>;
  try {
    got = host.open(root, path, file.controller.signal);
  } catch (err) {
    return queueMicrotask(() => failed(err));
  }
  Promise.resolve(got).then(read, failed);
}

export class Sys {
  readonly open = new Map<u32, File>();

  constructor(private readonly host: SysHost) {}

  /** For a program call to consume in place rather than READ into the guest. */
  bytes(fd: u32): U8Array | null {
    return this.open.get(fd)?.bytes ?? null;
  }

  run(memory: ArrayBuffer, at: u32): void {
    each_record(memory, at, HEAD, this.record);
  }

  private readonly record = (code: u32, w: U32Array, a: u32): void => {
    switch (code) {
      case op.OPENAT:
        open_file(this, this.host, w[a], w[a + 1], w[a + 2], text_of(w, a + 3));
        break;
      case op.READ: {
        const bytes = this.open.get(w[a])?.bytes;
        if (!bytes) throw new InvariantError(Errno.EBADF);
        const len = Math.min(w[a + 2], bytes.byteLength);
        new Uint8Array(w.buffer, w[a + 1], len).set(bytes.subarray(0, len));
        break;
      }
      case op.WRITE: {
        const written = this.open.get(w[a])?.written;
        if (!written) throw new InvariantError(Errno.EBADF);
        // A copy: the guest keeps the range only until its next turn.
        written.push(new Uint8Array(w.buffer, w[a + 1], w[a + 2]).slice());
        break;
      }
      case op.CLOSE: {
        const file = this.open.get(w[a]);
        if (!file) break;
        this.open.delete(w[a]);
        file.controller.abort();
        if (file.written) {
          const whole = new Uint8Array(file.written.reduce((n, b) => n + b.byteLength, 0));
          let off = 0;
          for (const b of file.written) {
            whole.set(b, off);
            off += b.byteLength;
          }
          this.host.commit(file.root, file.path, whole);
        }
        break;
      }
      case op.ABORT: {
        const num = (i: u32) => BigInt(w[a + i]) | BigInt(w[a + i + 1]) << 32n;
        this.host.abort({ errno: w[a] | 0, line: w[a + 1], column: w[a + 2], a: num(3), b: num(5), file: text_of(w, a + 7) });
        break;
      }
      default:
        if ((code & op.PROGRAM) === 0) throw new InvariantError(Errno.ENOSYS);
        this.host.program(code, w, a);
    }
  };
}
