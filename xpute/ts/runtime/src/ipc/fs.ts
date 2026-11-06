// @xpute/runtime/ipc/fs.ts

/**
 * The file device, Major FS (abi/fs.json): a core's opens, reads, writes
 * and closes, each a frame taken after the turn returns and before the next
 * begins, so whatever it names in core memory is written before the core
 * reads it. An open and a close are completed once each, by their tag, the
 * descriptor: an open with ECANCELED where its close came first. A write's
 * bytes are read where the core keeps them, at the close. What a file is,
 * the program's `Fs` says.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import {
  Command,
  encode_fs_openat_reply,
  type FsClose,
  type FsImports,
  type FsOpenat,
  type FsRead,
  type FsWrite,
} from "@xpute/runtime/abi/fs.spec.ts";
import type { Encode } from "./doorbell.ts";
import { flag } from "./fs.spec.ts";

/** Why an open did not complete with a file: an errno, and whatever number the
 * runtime refused with (an HTTP status) for a reader to print. */
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
   * aborted when the core closes it first. */
  open(
    root: u32,
    path: string,
    signal: AbortSignal,
  ): U8Array | Promise<U8Array>;
  /** All a file opened for writing was given, once it is closed; a
   * FileError where it could not be replaced. `bytes` are the runtime's. */
  commit(root: u32, path: string, bytes: U8Array): void;
}

/** A core's request completed: its tag and command, its result and the
 * completion's packet. */
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
  /** Its open is completed. */
  opened: boolean;
  bytes: U8Array | null;
  /** Where each write's bytes lie in the core's memory, in order. */
  written: [at: u32, len: u32][] | null;
}

/** No name empty, `.` or `..`, so a path never leaves its root. */
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

  /** `mem` is the core's, which a read writes and a close reads. */
  constructor(fs: Fs, mem: ArrayBufferLike, complete: Complete) {
    this._fs = fs;
    this._mem = mem as ArrayBuffer;
    this._complete = complete;
  }

  fs_openat(e: FsOpenat): void {
    this._openat(e.fd, e.root, e.flags, e.path);
  }

  fs_read(e: FsRead): void {
    const bytes = this.files.get(e.fd)?.bytes;
    if (!bytes) throw new InvariantError(Errno.EBADF);
    const len = Math.min(e.len, bytes.byteLength);
    new Uint8Array(this._mem, e.at, len).set(bytes.subarray(0, len));
  }

  /** A write to an open that failed is dropped: the core posts it before it
   * learns, and hears it from the open's completion and the close's. */
  fs_write(e: FsWrite): void {
    const file = this.files.get(e.fd);
    if (!file) return;
    if (!file.written) throw new InvariantError(Errno.EBADF);
    file.written.push([e.at, e.len]);
  }

  fs_close(e: FsClose): void {
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

  /** What `file` was written, read where the core keeps it and handed to
   * `Fs.commit`: 0, or the errno it refused with. */
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
    this._complete(fd, Command.FS_OPENAT, res, (room) =>
      encode_fs_openat_reply(room, detail),
    );
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
