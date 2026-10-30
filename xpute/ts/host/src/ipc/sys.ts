// @xpute/host/ipc/sys.ts

/**
 * The host's end of a guest's system calls (rust/guest/src/ipc/sys.rs):
 * the records a turn left, run after it returns and before the next begins, so
 * whatever they name in guest memory is written before the guest reads it; and
 * a program's call made as a hostcall, one record run before the hostcall
 * returns.
 */

import type { U32Array, U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import { Errno, strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { each_record, text_of } from "./stream.ts";
import { abort, args, flag, HEAD, op } from "./sys.spec.ts";

/** Why an open did not answer with a file: an errno, and whatever number the
 * host refused with (an HTTP status) for a reader to print. */
export class FileError extends Error {
  constructor(
    readonly errnum: Errno,
    readonly detail: u32 = 0,
  ) {
    super(detail === 0 ? strerror(errnum) : `${strerror(errnum)} (${detail})`);
  }
}

interface Abort {
  errnum: number;
  file: string;
  line: u32;
  column: u32;
  a: bigint;
  b: bigint;
}

export interface Fs {
  /** The file at `path` below `root`, whole, or a FileError; `signal` is
   * aborted when the guest closes it first. */
  open(
    root: u32,
    path: string,
    signal: AbortSignal,
  ): U8Array | Promise<U8Array>;
  /** All a file opened for writing was given, once it is closed. */
  commit(root: u32, path: string, bytes: U8Array): void;
}

export interface SysHost {
  fs: Fs;
  abort(report: Abort): void;
  /** A call of the program's own: its number, the record's words, and where
   * its arguments begin in them. A hostcall's answer is what it returns, 0
   * for none. */
  program(code: u32, w: U32Array, a: u32): i32 | void;
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
  return path
    .split("/")
    .every((name) => name !== "" && name !== "." && name !== "..");
}

export class Sys {
  readonly files = new Map<u32, File>();

  /** `opened` answers an open: a size, or an errno and the host's detail. */
  constructor(
    private readonly host: SysHost,
    private readonly opened: (fd: u32, res: i32, detail: u32) => void,
  ) {}

  run(mem: ArrayBuffer, at: u32): void {
    each_record(mem, at, HEAD, this.record);
  }

  /** The hostcall's record at `at`, run now: the program's answer, or ENOSYS
   * for a call that is not the program's. */
  hostcall(mem: ArrayBuffer, at: u32): i32 {
    const [code, n] = new Uint32Array(mem, at, 2);
    if ((code & op.PROGRAM) === 0) return Errno.ENOSYS;
    const w = new Uint32Array(mem, at, 2 + n) as U32Array;
    return this.host.program(code, w, 2) ?? 0;
  }

  private readonly record = (code: u32, w: U32Array, a: u32): void => {
    switch (code) {
      case op.OPENAT:
        this.openat(w[a], w[a + 1], w[a + 2], text_of(w, a + args.OPENAT));
        break;
      case op.READ: {
        const bytes = this.files.get(w[a])?.bytes;
        if (!bytes) throw new InvariantError(Errno.EBADF);
        const len = Math.min(w[a + 2], bytes.byteLength);
        new Uint8Array(w.buffer, w[a + 1], len).set(bytes.subarray(0, len));
        break;
      }
      case op.WRITE: {
        const written = this.files.get(w[a])?.written;
        if (!written) throw new InvariantError(Errno.EBADF);
        // A copy: the guest keeps the range only until its next turn.
        written.push(new Uint8Array(w.buffer, w[a + 1], w[a + 2]).slice());
        break;
      }
      case op.CLOSE: {
        const file = this.files.get(w[a]);
        if (!file) break;
        this.files.delete(w[a]);
        file.controller.abort();
        if (file.written) {
          const whole = new Uint8Array(
            file.written.reduce((n, b) => n + b.byteLength, 0),
          );
          let off = 0;
          for (const b of file.written) {
            whole.set(b, off);
            off += b.byteLength;
          }
          this.host.fs.commit(file.root, file.path, whole);
        }
        break;
      }
      case op.ABORT: {
        const num = (i: u32) =>
          BigInt(w[a + i]) | (BigInt(w[a + i + 1]) << 32n);
        this.host.abort({
          errnum: w[a] | 0,
          line: w[a + 1],
          column: w[a + 2],
          a: num(abort.A),
          b: num(abort.B),
          file: text_of(w, a + args.ABORT),
        });
        break;
      }
      default:
        if ((code & op.PROGRAM) === 0) throw new InvariantError(Errno.ENOSYS);
        this.host.program(code, w, a);
    }
  };

  private openat(fd: u32, root: u32, flags: u32, path: string): void {
    const file: File = {
      root,
      path,
      controller: new AbortController(),
      bytes: null,
      written: null,
    };
    this.files.set(fd, file);
    // An answer is a submission, and a full ring rings a turn that would
    // overwrite the records this run is still reading, so answers wait until it ends.
    const answer = (res: i32, detail: u32) => {
      if (this.files.get(fd) !== file) return;
      if (res < 0) this.files.delete(fd);
      this.opened(fd, res, detail);
    };
    const failed = (err: unknown) => {
      if (!(err instanceof FileError)) {
        console.warn(
          `[xpute/sys] open ${root}/${path}: ${
            err instanceof Error ? err.message : String(err)
          }`,
        );
      }
      const [errnum, detail] =
        err instanceof FileError ? [err.errnum, err.detail] : [Errno.EIO, 0];
      answer(errnum, detail);
    };
    const read = (bytes: U8Array) => {
      if (this.files.get(fd) !== file) return;
      file.bytes = bytes;
      answer(bytes.byteLength, 0);
    };
    if (!walks_down(path)) {
      return queueMicrotask(() => failed(new FileError(Errno.EACCES)));
    }
    if (flags === flag.O_WRONLY) {
      file.written = [];
      return queueMicrotask(() => answer(0, 0));
    }
    let got: U8Array | Promise<U8Array>;
    try {
      got = this.host.fs.open(root, path, file.controller.signal);
    } catch (err) {
      return queueMicrotask(() => failed(err));
    }
    Promise.resolve(got).then(read, failed);
  }
}
