// @xpute/host/ipc/sys.spec.ts
//
// GENERATED from spec/ipc/sys.json — do not edit.
//
// A guest's system calls: its files, the report that stopped it and the program's own
// calls, as records in a stream (ipc/stream.rs) the host runs once each turn has returned
// and before the next begins. A record may name memory the guest holds: the host reads or
// writes it in that gap, and the guest keeps it that long. The directory says where the
// stream is (key SYS).
//
// Files borrow Plan 9's file namespace, in POSIX's words. The guest issues the descriptor,
// a generational handle, and names a file by a root the program numbers and a path below
// it; the program binds each root to where it keeps things, and a path that leaves it is
// refused. OPENAT is the one call that waits: the host answers it with the command
// SYS_OPENED (abi/cmd.json) once the file can be read whole. READ and WRITE are done in
// the gap; CLOSE ends the descriptor, opened or not, and nothing is said of it after.
//
// A program's own calls go in the same stream, numbered with the PROGRAM bit, and a host
// that is not the program's refuses them.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** Words before the first record: END, then a word kept at 0. */
export const HEAD: u32 = 2 as number;

/** A record's call, then its arguments. */
export const op = {
  /** fd, root, flags, then the path's bytes: the file at `path` below `root` opened as `fd`,
   * answered by SYS_OPENED with its size or -errno. */
  OPENAT: 1 as number,
  /** fd, at, len: the opened file's first `len` bytes copied to `at`. */
  READ: 2 as number,
  /** fd, at, len: the `len` bytes at `at` added to what the file opened for writing holds. */
  WRITE: 3 as number,
  /** fd: the descriptor ended. A file opened for writing is replaced by what was written; an
   * open not yet answered is canceled, and never answered. */
  CLOSE: 4 as number,
  /** errno, line, column, a (low, high word), b (low, high word), then the file's bytes:
   * the broken invariant the guest reports before it stops (xpute-kit status/bug.rs). */
  ABORT: 5 as number,
  /** The bit every call a program numbers for itself carries. */
  PROGRAM: 2147483648 as number,
} as const;

/** How many words of arguments each call's record takes, as its `op` lists them; OPENAT's
 * and ABORT's end in bytes after them. */
export const args = {
  OPENAT: 3 as number,
  READ: 3 as number,
  WRITE: 3 as number,
  CLOSE: 1 as number,
  ABORT: 7 as number,
} as const;

/** Where ABORT's two values lie among its arguments, each low word then high. */
export const abort = {
  /** a. */
  A: 3 as number,
  /** b. */
  B: 5 as number,
} as const;

/** How OPENAT opens. */
export const flag = {
  /** To read, whole. */
  O_RDONLY: 0 as number,
  /** To write: created where it is not, and replaced whole at CLOSE (O_CREAT | O_TRUNC). */
  O_WRONLY: 1 as number,
} as const;
