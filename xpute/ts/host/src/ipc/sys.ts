// @xpute/host/ipc/sys.ts

/**
 * The host's port, 0, as a guest's hostcalls reach it (ipc/sys.json): each a
 * frame on the completion ring, taken after the turn returns and before the
 * next begins, so whatever it names in guest memory is written before the
 * guest reads it.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import {
  Command,
  read_sys_abort,
  read_sys_close,
  read_sys_openat,
  read_sys_read,
  read_sys_write,
} from "@xpute/host/abi/cmd.spec.ts";
import { flag } from "./sys.spec.ts";

/** Why an open did not answer with a file: an errno, and whatever number the
 * host refused with (an HTTP status) for a reader to print. */
export class FileError extends Error {
  readonly errnum: i32;
  readonly detail: u32;

  constructor(errnum: i32, detail: u32 = 0) {
    super(detail === 0 ? strerror(errnum) : `${strerror(errnum)} (${detail})`);
    this.errnum = errnum;
    this.detail = detail;
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

/** What answers a guest's hostcalls. Each part a program leaves out is
 * refused: an open with ENOENT (a write's with EROFS); a report it does not
 * take is logged. */
export interface SysHost {
  fs?: Fs;
  onabort?(report: Abort): void;
}

function log_abort(r: Abort): void {
  console.error(
    `[xpute] stopped: ${strerror(r.errnum)} at ${r.file}:${r.line}:${r.column} (${r.a}, ${r.b})`,
  );
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

  private readonly host: SysHost;
  private readonly mem: ArrayBuffer;
  private readonly answer_open: (fd: u32, res: i32, detail: u32) => void;

  /** `mem` is the guest's, which a read writes and a write reads;
   * `answer_open` answers an open: a size, or an errno and the host's detail. */
  constructor(
    host: SysHost,
    mem: ArrayBufferLike,
    answer_open: (fd: u32, res: i32, detail: u32) => void,
  ) {
    this.host = host;
    this.mem = mem as ArrayBuffer;
    this.answer_open = answer_open;
  }

  /** A hostcall the guest sent the host's port (doorbell.ts `on`). */
  readonly receive = (_tag: u32, cmd: u32, f: BranchCursor | null): void => {
    if (f === null) throw new InvariantError(Errno.EBADMSG);
    switch (cmd) {
      case Command.SYS_OPENAT: {
        const c = read_sys_openat(f);
        this.openat(c.fd, c.root, c.flags, c.path);
        break;
      }
      case Command.SYS_READ: {
        const c = read_sys_read(f);
        const bytes = this.files.get(c.fd)?.bytes;
        if (!bytes) throw new InvariantError(Errno.EBADF);
        const len = Math.min(c.len, bytes.byteLength);
        new Uint8Array(this.mem, c.at, len).set(bytes.subarray(0, len));
        break;
      }
      case Command.SYS_WRITE: {
        const c = read_sys_write(f);
        const written = this.files.get(c.fd)?.written;
        if (!written) throw new InvariantError(Errno.EBADF);
        // A copy: the guest keeps the range only until its next turn.
        written.push(new Uint8Array(this.mem, c.at, c.len).slice());
        break;
      }
      case Command.SYS_CLOSE: {
        const fd = read_sys_close(f).fd;
        const file = this.files.get(fd);
        if (!file) break;
        this.files.delete(fd);
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
          this.host.fs?.commit(file.root, file.path, whole);
        }
        break;
      }
      case Command.SYS_ABORT: {
        const c = read_sys_abort(f);
        const num = (low: u32, high: u32) =>
          BigInt(low) | (BigInt(high) << 32n);
        (this.host.onabort ?? log_abort)({
          errnum: c.errnum,
          line: c.line,
          column: c.column,
          a: num(c.a_low, c.a_high),
          b: num(c.b_low, c.b_high),
          file: c.file,
        });
        break;
      }
      default:
        throw new InvariantError(Errno.ENOSYS);
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
    // An answer is a submission, and a full ring rings a turn, which the
    // completion ring being read forbids, so answers wait until it ends.
    const answer = (res: i32, detail: u32) => {
      if (this.files.get(fd) !== file) return;
      if (res < 0) this.files.delete(fd);
      this.answer_open(fd, res, detail);
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
    const fs = this.host.fs;
    if (!fs) {
      const errnum = flags === flag.O_WRONLY ? Errno.EROFS : Errno.ENOENT;
      return queueMicrotask(() => failed(new FileError(errnum)));
    }
    if (flags === flag.O_WRONLY) {
      file.written = [];
      return queueMicrotask(() => answer(0, 0));
    }
    let got: U8Array | Promise<U8Array>;
    try {
      got = fs.open(root, path, file.controller.signal);
    } catch (err) {
      return queueMicrotask(() => failed(err));
    }
    Promise.resolve(got).then(read, failed);
  }
}
