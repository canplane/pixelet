// @xpute/host/ipc/fs.ts

/**
 * The file device, Major FS (abi/fs.json): a guest's opens, reads, writes
 * and closes, each a frame taken after the turn returns and before the next
 * begins, so whatever it names in guest memory is written before the guest
 * reads it. What a file is, the program's `Fs` says.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import type {
  FsCloseEvent,
  FsImports,
  FsOpenatEvent,
  FsReadEvent,
  FsWriteEvent,
} from "@xpute/host/abi/fs.spec.ts";
import { flag } from "./fs.spec.ts";

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

/** What backs the files: an open's bytes, and a written file's commit. */
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

export class FsServer implements FsImports {
  readonly files = new Map<u32, File>();

  private readonly fs: Fs;
  private readonly mem: ArrayBuffer;
  private readonly answer_open: (fd: u32, res: i32, detail: u32) => void;

  /** `mem` is the guest's, which a read writes and a write reads;
   * `answer_open` answers an open: a size, or an errno and the host's detail. */
  constructor(
    fs: Fs,
    mem: ArrayBufferLike,
    answer_open: (fd: u32, res: i32, detail: u32) => void,
  ) {
    this.fs = fs;
    this.mem = mem as ArrayBuffer;
    this.answer_open = answer_open;
  }

  fs_openat(e: FsOpenatEvent): void {
    this.openat(e.fd, e.root, e.flags, e.path);
  }

  fs_read(e: FsReadEvent): void {
    const bytes = this.files.get(e.fd)?.bytes;
    if (!bytes) throw new InvariantError(Errno.EBADF);
    const len = Math.min(e.len, bytes.byteLength);
    new Uint8Array(this.mem, e.at, len).set(bytes.subarray(0, len));
  }

  fs_write(e: FsWriteEvent): void {
    const written = this.files.get(e.fd)?.written;
    if (!written) throw new InvariantError(Errno.EBADF);
    // A copy: the guest keeps the range only until its next turn.
    written.push(new Uint8Array(this.mem, e.at, e.len).slice());
  }

  fs_close(e: FsCloseEvent): void {
    const file = this.files.get(e.fd);
    if (!file) return;
    this.files.delete(e.fd);
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
      this.fs.commit(file.root, file.path, whole);
    }
  }

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
          `[xpute/fs] open ${root}/${path}: ${
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
      got = this.fs.open(root, path, file.controller.signal);
    } catch (err) {
      return queueMicrotask(() => failed(err));
    }
    Promise.resolve(got).then(read, failed);
  }
}
