// @xpute/runtime/ipc/sys.ts

/**
 * The host's end of a guest's system calls (crates/xpute/runtime/src/ipc/
 * sys.rs, sys.spec.ts): the records a turn left in the stream the directory
 * names under SYS, run once the turn has returned and before the next begins,
 * so whatever one names in the guest's memory is written by the time the
 * guest reads it.
 *
 * Files are run here: the descriptors the guest issued, each a file opened
 * whole below a root, read, written and closed. What a root is — storage, an
 * origin over HTTP — is the program's host to answer (`SysHost.open`,
 * `commit`), and so is how an open's answer reaches the guest (`opened`); a
 * call numbered with the PROGRAM bit is handed to it whole.
 */

import type { U32Array, U8Array } from "@xpute/core/abi/array.ts";
import type { i32, u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { InvariantError } from "@xpute/core/status/error.ts";
import { bytes_of, each_record } from "./stream.ts";
import { flag, HEAD, op } from "./sys.spec.ts";

/** Why an open did not answer with a file: an errno, and whatever number the
 * host refused with (an HTTP status) for a reader to print. */
export class FileError extends Error {
  constructor(readonly errno: Errno, readonly detail: u32 = 0) {
    super(`errno ${errno}`);
  }
}

/** A broken invariant the guest reported before it stopped. */
export interface Abort {
  errno: number;
  file: string;
  line: u32;
  column: u32;
  a: bigint;
  b: bigint;
}

/** What only the program's host can answer. */
export interface SysHost {
  /** The file at `path` below `root`, whole, or a FileError; `signal` is
   * aborted when the guest closes it first. */
  open(root: u32, path: string, signal: AbortSignal): U8Array | Promise<U8Array>;
  /** `bytes` replace the file at `path` below `root`. */
  commit(root: u32, path: string, bytes: U8Array): void;
  /** An open answered: the size, or an errno (negative). */
  opened(fd: u32, res: i32, detail: u32): void;
  /** What stopped the guest. */
  abort(report: Abort): void;
  /** A call of the program's own: its number, the record's words, and where
   * its arguments begin in them. */
  program(code: u32, w: U32Array, a: u32): void;
}

interface File {
  root: u32;
  path: string;
  /** Aborts an open still waiting. */
  controller: AbortController;
  /** The file, once opened to read. */
  bytes: U8Array | null;
  /** What was written, where opened to write. */
  written: U8Array[] | null;
}

const utf8 = new TextDecoder();

/** The descriptors a guest holds open, by the handle it issued. */
export class Files {
  readonly open = new Map<u32, File>();
}

function open_file(files: Files, host: SysHost, fd: u32, root: u32, flags: u32, path: string): void {
  const file: File = { root, path, controller: new AbortController(), bytes: null, written: null };
  files.open.set(fd, file);
  if (flags === flag.O_WRONLY) {
    file.written = [];
    host.opened(fd, 0, 0);
    return;
  }
  // Answered once the record run is over, so an answer at once and one a
  // response later reach the guest the same way.
  void (async () => {
    await Promise.resolve();
    try {
      const bytes = await host.open(root, path, file.controller.signal);
      if (files.open.get(fd) !== file) return;
      file.bytes = bytes;
      host.opened(fd, bytes.byteLength, 0);
    } catch (err) {
      if (files.open.get(fd) !== file) return;
      files.open.delete(fd);
      const [errno, detail] = err instanceof FileError ? [err.errno, err.detail] : [Errno.EIO, 0];
      if (!(err instanceof FileError)) console.warn(`[xpute/sys] open ${root}/${path}: ${err instanceof Error ? err.message : String(err)}`);
      host.opened(fd, errno, detail);
    }
  })();
}

/** Runs the calls the turn just returned left in the stream at `at`. */
export function run_sys(memory: ArrayBuffer, at: u32, files: Files, host: SysHost): void {
  each_record(memory, at, HEAD, (code, w, a) => {
    switch (code) {
      case op.OPENAT:
        open_file(files, host, w[a], w[a + 1], w[a + 2], utf8.decode(bytes_of(w, a + 3)));
        break;
      case op.READ: {
        const bytes = files.open.get(w[a])?.bytes;
        if (!bytes) throw new InvariantError(Errno.EBADF);
        const len = Math.min(w[a + 2], bytes.byteLength);
        new Uint8Array(memory, w[a + 1], len).set(bytes.subarray(0, len));
        break;
      }
      case op.WRITE: {
        const written = files.open.get(w[a])?.written;
        if (!written) throw new InvariantError(Errno.EBADF);
        // A copy: the guest keeps the range only until its next turn.
        written.push(new Uint8Array(memory, w[a + 1], w[a + 2]).slice());
        break;
      }
      case op.CLOSE: {
        const file = files.open.get(w[a]);
        if (!file) break;
        files.open.delete(w[a]);
        file.controller.abort();
        if (file.written) {
          const whole = new Uint8Array(file.written.reduce((n, b) => n + b.byteLength, 0));
          let off = 0;
          for (const b of file.written) {
            whole.set(b, off);
            off += b.byteLength;
          }
          host.commit(file.root, file.path, whole);
        }
        break;
      }
      case op.ABORT: {
        const num = (i: u32) => BigInt(w[a + i]) | BigInt(w[a + i + 1]) << 32n;
        host.abort({ errno: w[a] | 0, line: w[a + 1], column: w[a + 2], a: num(3), b: num(5), file: utf8.decode(bytes_of(w, a + 7)) });
        break;
      }
      default:
        if ((code & op.PROGRAM) === 0) throw new InvariantError(Errno.ENOSYS);
        host.program(code, w, a);
    }
  });
}
