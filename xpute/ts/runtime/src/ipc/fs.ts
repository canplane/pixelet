// @xpute/runtime/ipc/fs.ts

/**
 * This is the file device on Major FS (spec/abi/fs.json): the runtime's server
 * for a core's opens, reads, writes and closes. Each is a frame taken after the
 * turn returns and before the next begins, so whatever the frame names in the
 * core's memory is written before the core reads it. An open and a close are
 * each completed once, by their tag, which is the descriptor; an open whose
 * close came first completes with ECANCELED. A write's bytes stay where the
 * core keeps them, and are read from there at the close. The program's `Fs`
 * says what a file is.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import {
  Command,
  type FsClose,
  type FsImports,
  type FsOpenat,
  type FsRead,
  type FsWrite,
} from "@xpute/runtime/abi/fs.spec.ts";
import { type Encode, encode_args } from "./doorbell.ts";
import { flag } from "./fs.spec.ts";

/** Why an open did not complete with a file: an errno, and whatever number
 * the store behind the file refused with (for HTTP, its status), kept for a
 * reader to print. */
export class FileError extends Error {
  readonly errnum: i32;
  readonly detail: u32;

  constructor(errnum: i32, detail: u32 = 0) {
    super(detail === 0 ? strerror(errnum) : `${strerror(errnum)} (${detail})`);
    this.errnum = errnum;
    this.detail = detail;
  }
}

/** An `Fs` backs the files: an open's bytes, and a written file's commit. */
export interface Fs {
  /** Returns the whole file at `path` below `root`, or throws a FileError.
   * `signal` is aborted when the core closes the file before its open
   * completes. */
  open(
    root: u32,
    path: string,
    signal: AbortSignal,
  ): U8Array | Promise<U8Array>;
  /** Receives all the bytes a file opened for writing was given, once it is
   * closed, and throws a FileError where the file could not be replaced.
   * `bytes` lies in the runtime's own memory, not the core's, so it stays
   * valid after the call. */
  commit(root: u32, path: string, bytes: U8Array): void;
  /** Where the bytes of the file at `path` below `root` can be fetched from,
   * for a runtime with a `fetcher`; none when only `open` answers. */
  url?(root: u32, path: string): string | undefined;
  /** Where a device fetches a file with no JavaScript on the bytes' way. */
  fetcher?: Fetcher;
}

/** Fetches a file off the JavaScript thread and hands its bytes to the runtime
 * itself. */
export interface Fetcher {
  /** Resolves once the bytes have gone to the runtime, and rejects with why
   * they did not. */
  fetch(fd: u32, url: string): Promise<void>;
  cancel(fd: u32): void;
}

/** A `Complete` completes a core's request, given its tag and command, the
 * result, and the completion's packet. */
export type Complete = (
  tag: u32,
  cmd: u32,
  res: i32,
  encode: Encode | null,
) => void;

interface File {
  root: u32;
  path: string;
  controller: AbortController;
  /** This says whether its open has completed. */
  opened: boolean;
  bytes: U8Array | null;
  /** Where each write's bytes lie in the core's memory, in order. */
  written: [at: u32, len: u32][] | null;
}

/** Whether no name in `path` is empty, `.` or `..`, so that the path cannot
 * leave its root. */
function walks_down(path: string): boolean {
  return path
    .split("/")
    .every((name) => name !== "" && name !== "." && name !== "..");
}

export class FsServer implements FsImports {
  readonly files = new Map<u32, File>();

  private readonly _fs: Fs;
  private readonly _mem: ArrayBuffer;
  private readonly _complete: Complete;

  /** `mem` is the core's memory, which a read writes into and a close reads
   * from. */
  constructor(fs: Fs, mem: ArrayBufferLike, complete: Complete) {
    this._fs = fs;
    this._mem = mem as ArrayBuffer;
    this._complete = complete;
  }

  [Command.FS_OPENAT](e: FsOpenat): void {
    this._openat(e.fd, e.root, e.flags, e.path);
  }

  /** A read of a file this runtime does not hold is dropped, as a write is.
   * The runtime a reload replaced left it in the ring. */
  [Command.FS_READ](e: FsRead): void {
    const file = this.files.get(e.fd);
    if (!file) return;
    if (!file.bytes) throw new InvariantError(Errno.EBADF);
    const len = Math.min(e.len, file.bytes.byteLength);
    new Uint8Array(this._mem, e.at, len).set(file.bytes.subarray(0, len));
  }

  /** A write to a file whose open failed is dropped. The core posts the
   * write before it learns of the failure, and it hears of the failure from
   * the open's completion and the close's. */
  [Command.FS_WRITE](e: FsWrite): void {
    const file = this.files.get(e.fd);
    if (!file) return;
    if (!file.written) throw new InvariantError(Errno.EBADF);
    file.written.push([e.at, e.len]);
  }

  [Command.FS_CLOSE](e: FsClose): void {
    const file = this.files.get(e.fd);
    if (!file) {
      this._complete(e.fd, Command.FS_CLOSE, Errno.EBADF, null);
      return;
    }
    this.files.delete(e.fd);
    if (!file.opened) {
      file.controller.abort();
      this._complete_open(e.fd, Errno.ECANCELED, 0);
    }
    this._complete(e.fd, Command.FS_CLOSE, this._commit(file), null);
  }

  /** Reads what `file` was written from where the core keeps it, and hands it
   * to `Fs.commit`. It returns 0, or the errno the commit refused with. */
  private _commit(file: File): i32 {
    if (!file.written) return Errno.OK;
    const whole = new Uint8Array(
      file.written.reduce((n, [, len]) => n + len, 0),
    );
    let off = 0;
    for (const [at, len] of file.written) {
      whole.set(new Uint8Array(this._mem, at, len), off);
      off += len;
    }
    try {
      this._fs.commit(file.root, file.path, whole);
      return Errno.OK;
    } catch (err) {
      if (err instanceof FileError) return err.errnum;
      console.warn(
        `[xpute/fs] commit ${file.root}/${file.path}: ${
          err instanceof Error ? err.message : String(err)
        }`,
      );
      return Errno.EIO;
    }
  }

  private _complete_open(fd: u32, res: i32, detail: u32): void {
    this._complete(fd, Command.FS_OPENAT, res, encode_args([detail]));
  }

  private _openat(fd: u32, root: u32, flags: u32, path: string): void {
    const file: File = {
      root,
      path,
      controller: new AbortController(),
      opened: false,
      bytes: null,
      written: null,
    };
    this.files.set(fd, file);
    const answer = (res: i32, detail: u32) => {
      if (this.files.get(fd) !== file) return;
      file.opened = true;
      if (res < 0) this.files.delete(fd);
      this._complete_open(fd, res, detail);
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
    if (!walks_down(path)) return failed(new FileError(Errno.EACCES));
    if (flags === flag.O_WRONLY) {
      file.written = [];
      return answer(0, 0);
    }
    let got: U8Array | Promise<U8Array>;
    try {
      got = this._fs.open(root, path, file.controller.signal);
    } catch (err) {
      return failed(err);
    }
    const read = (bytes: U8Array) => {
      if (this.files.get(fd) !== file) return;
      file.bytes = bytes;
      answer(bytes.byteLength, 0);
    };
    if (got instanceof Promise) got.then(read, failed);
    else read(got);
  }
}
