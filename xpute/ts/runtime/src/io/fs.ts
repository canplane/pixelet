// @xpute/runtime/io/fs.ts

/**
 * This is the file device on Major FS (spec/abi/fs.json): the runtime's server
 * for a core's opens, reads, writes and closes. Each is a frame taken after
 * the turn returns and before the next begins, so whatever a read fills in the
 * core's memory is written before the core reads it. An open and a close are
 * each completed once, by their tag, which is the descriptor; an open whose
 * close came first completes with ECANCELED. An open to write completes at
 * once, each write's bytes are kept as its frame comes, and the close hands
 * the whole file to the binding (`commit`). A read of a file opened to write,
 * or a write of one opened to read, leaves the file alone, and its close
 * completes with EBADF. The program's `Binding` says what a file is.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import {
  Command,
  type FsImports,
  type FsOpen,
  type FsRead,
  type FsWrite,
} from "@xpute/runtime/abi/fs.spec.ts";
import { type Encode, encode_args } from "./connection.ts";
import { in_core } from "./frame.ts";
import { flag } from "./fs.spec.ts";
import type { Path } from "./path.ts";

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

/** The program's binding answers every path its core opens or writes; what a
 * path names is for it alone to decide. `open` returns the whole file at a
 * path, or throws a FileError; its `signal` is aborted when the core closes
 * the file before its open completes. `commit` receives the whole file a core
 * wrote, at its close, and throws a FileError where the file could not be
 * replaced, whose errno the close completes with, as a filesystem answers
 * EROFS for a file it does not write; the bytes are the runtime's copy, not
 * the core's memory, so they stay valid after the call. */
export interface Binding {
  open(path: Path, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit(path: Path, bytes: U8Array): void;
}

/** Replaces the file at `path` with the bytes written to it, in the order
 * written, and answers the close's result: 0, the errno the binding refused
 * the file with, or EIO where it failed otherwise. */
export function commit(
  binding: Binding,
  path: Path,
  written: readonly U8Array[],
): i32 {
  let len = 0;
  for (const chunk of written) len += chunk.byteLength;
  const bytes = new Uint8Array(len);
  let at = 0;
  for (const chunk of written) {
    bytes.set(chunk, at);
    at += chunk.byteLength;
  }
  try {
    binding.commit(path, bytes);
    return Errno.OK;
  } catch (err) {
    if (err instanceof FileError) return err.errnum;
    console.warn(
      `[xpute/fs] commit ${JSON.stringify(path)}: ${
        err instanceof Error ? err.message : String(err)
      }`,
    );
    return Errno.EIO;
  }
}

/** A `Complete` completes a core's request, given its tag and command, the
 * result, and the completion's packet. */
export type Complete = (
  tag: u32,
  cmd: u32,
  res: i32,
  encode: Encode | null,
) => void;

/** A file the core holds open, by how it was opened. Its `err` is the errno
 * its close completes with, or 0: EFAULT after a read outside the core's
 * memory, EBADF after a read or a write against how it was opened. A file
 * read holds its bytes once its open completes; a file written, its path
 * and what was written to it, each write's bytes a copy. */
type File =
  | {
      kind: "read";
      controller: AbortController;
      bytes: U8Array | null;
      err: i32;
    }
  | { kind: "write"; path: Path; written: U8Array[]; err: i32 };

export class FsServer implements FsImports {
  private readonly _files = new Map<u32, File>();

  private readonly _binding: Binding;
  private readonly _mem: ArrayBuffer;
  private readonly _complete: Complete;

  /** `mem` is the core's memory, which a read writes into. */
  constructor(binding: Binding, mem: ArrayBufferLike, complete: Complete) {
    this._binding = binding;
    this._mem = mem as ArrayBuffer;
    this._complete = complete;
  }

  /** An open whose flag is neither O_RDONLY nor O_WRONLY completes with
   * EINVAL, and so does its descriptor's close, as no read or write of it
   * reaches a file. */
  [Command.FS_OPEN](fd: u32, e: FsOpen): void {
    if (e.flags === flag.O_RDONLY) return this._open(fd, e.path);
    if (e.flags === flag.O_WRONLY) return this._create(fd, e.path);
    this._files.set(fd, {
      kind: "read",
      controller: new AbortController(),
      bytes: new Uint8Array(0),
      err: Errno.EINVAL,
    });
    this._complete_open(fd, Errno.EINVAL, 0);
  }

  /** A read of a file this runtime does not hold is dropped. The runtime a
   * reload replaced left it unserved. A read into memory that is not the
   * core's (`in_core`) copies nothing, and its descriptor's close completes
   * with EFAULT. */
  [Command.FS_READ](fd: u32, e: FsRead): void {
    const file = this._files.get(fd);
    if (!file) return;
    if (file.kind !== "read" || file.bytes === null) {
      return fail(file, Errno.EBADF);
    }
    if (!in_core(this._mem, e.at, e.len)) return fail(file, Errno.EFAULT);
    const len = Math.min(e.len, file.bytes.byteLength);
    new Uint8Array(this._mem, e.at, len).set(file.bytes.subarray(0, len));
  }

  /** A write of a file this runtime does not hold is dropped, as a read
   * is. */
  [Command.FS_WRITE](fd: u32, _e: FsWrite, named: U8Array): void {
    const file = this._files.get(fd);
    if (!file) return;
    if (file.kind !== "write") return fail(file, Errno.EBADF);
    file.written.push(named.slice());
  }

  [Command.FS_CLOSE](fd: u32): void {
    const file = this._files.get(fd);
    if (!file) {
      this._complete(fd, Command.FS_CLOSE, Errno.EBADF, null);
      return;
    }
    this._files.delete(fd);
    if (file.kind === "read" && file.bytes === null) {
      file.controller.abort();
      this._complete_open(fd, Errno.ECANCELED, 0);
    }
    const res =
      file.err !== Errno.OK || file.kind === "read"
        ? file.err
        : commit(this._binding, file.path, file.written);
    this._complete(fd, Command.FS_CLOSE, res, null);
  }

  private _complete_open(fd: u32, res: i32, detail: u32): void {
    this._complete(fd, Command.FS_OPEN, res, encode_args([detail]));
  }

  /** Opens the file at `path` to write, which completes at once: nothing
   * reaches the binding before the close. */
  private _create(fd: u32, path: Path): void {
    this._files.set(fd, { kind: "write", path, written: [], err: Errno.OK });
    this._complete_open(fd, Errno.OK, 0);
  }

  private _open(fd: u32, path: Path): void {
    const file: File = {
      kind: "read",
      controller: new AbortController(),
      bytes: null,
      err: Errno.OK,
    };
    this._files.set(fd, file);
    const answer = (res: i32, detail: u32) => {
      if (this._files.get(fd) !== file) return;
      if (res < 0) this._files.delete(fd);
      this._complete_open(fd, res, detail);
    };
    const failed = (err: unknown) => {
      if (!(err instanceof FileError)) {
        console.warn(
          `[xpute/fs] open ${JSON.stringify(path)}: ${
            err instanceof Error ? err.message : String(err)
          }`,
        );
      }
      const [errnum, detail] =
        err instanceof FileError ? [err.errnum, err.detail] : [Errno.EIO, 0];
      answer(errnum, detail);
    };
    let got: U8Array | Promise<U8Array>;
    try {
      got = this._binding.open(path, file.controller.signal);
    } catch (err) {
      return failed(err);
    }
    const read = (bytes: U8Array) => {
      if (this._files.get(fd) !== file) return;
      file.bytes = bytes;
      answer(bytes.byteLength, 0);
    };
    if (got instanceof Promise) got.then(read, failed);
    else read(got);
  }
}

/** Marks `file`'s close to complete with `errnum`, unless an errno already
 * waits. */
function fail(file: File, errnum: i32): void {
  if (file.err === Errno.OK) file.err = errnum;
}
