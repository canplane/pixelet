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
import { each_record, text_of } from "./stream.ts";
import { flag, HEAD, op } from "./sys.spec.ts";

/** Why an open did not answer with a file: an errno, and whatever number the
 * host refused with (an HTTP status) for a reader to print. */
export class FileError extends Error {
  constructor(readonly errno: Errno, readonly detail: u32 = 0) {
    super(`errno ${errno}`);
  }
}

/** A broken invariant the guest reported before it stopped. */
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

/** Whether `path` walks down from its root and nowhere else, as 9P's walk
 * does a name at a time: no name empty, `.` or `..`, so no path leaves the
 * root it is opened below and the root is the whole of what it grants. */
function walks_down(path: string): boolean {
  return path.split("/").every((name) => name !== "" && name !== "." && name !== "..");
}

function open_file(files: Sys, host: SysHost, fd: u32, root: u32, flags: u32, path: string): void {
  const file: File = { root, path, controller: new AbortController(), bytes: null, written: null };
  files.open.set(fd, file);
  // Every answer leaves the record run first, even one known at once: an
  // answer is a submission, and a submission the ring has no room for rings
  // a turn to make room — a turn inside this run would write over the
  // records it is still reading. Asked when it is sent, so a descriptor
  // closed in the same run is answered nothing.
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

/** A guest's system calls as its host runs them: the descriptors it holds
 * open, by the handle it issued, and the host that answers what only it can. */
export class Sys {
  readonly open = new Map<u32, File>();

  constructor(private readonly host: SysHost) {}

  /** The bytes of the file opened to read as `fd`, for a call of the
   * program's own to consume where the host is, rather than copied into the
   * guest by a READ: none where `fd` holds no such file. */
  bytes(fd: u32): U8Array | null {
    return this.open.get(fd)?.bytes ?? null;
  }

  /** Runs the calls the turn just returned left in the stream at `at`. */
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
