// @xpute/host/ipc/sys/op.spec.ts
//
// GENERATED from spec/ipc/sys.json

/** fd, root, flags, then the path's bytes: the file at `path` below `root` opened as `fd`,
 * answered by SYS_OPENED with its size or -errno. */
export const OPENAT = 1 as number;

/** fd, at, len: the opened file's first `len` bytes copied to `at`. */
export const READ = 2 as number;

/** fd, at, len: the `len` bytes at `at` added to what the file opened for writing holds. */
export const WRITE = 3 as number;

/** fd: the descriptor ended. A file opened for writing is replaced by what was written; an
 * open not yet answered is canceled, and never answered. */
export const CLOSE = 4 as number;

/** errno, line, column, a (low, high word), b (low, high word), then the file's bytes:
 * the broken invariant the guest reports before it stops (xpute-kit status/bug.rs). */
export const ABORT = 5 as number;

/** The bit every call a program numbers for itself carries. */
export const PROGRAM = 2147483648 as number;
