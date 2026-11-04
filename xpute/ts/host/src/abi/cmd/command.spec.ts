// @xpute/host/abi/cmd/command.spec.ts
//
// GENERATED from spec/abi/cmd.json

// xpute's own commands.

/** Replies with its packet's value, or 0 where it has none — the rings' own test. */
export const NOP = 0x0000;
/** An OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
 * Never sent for a descriptor already closed. */
export const SYS_OPENED = 0x0101;
/** The file at `path` below `root` opened as `fd`, answered by SYS_OPENED. */
export const SYS_OPENAT = 0x0102;
/** The opened file's first `len` bytes copied to `at` in the guest's memory. */
export const SYS_READ = 0x0103;
/** The `len` bytes at `at` added to what the file opened for writing holds. */
export const SYS_WRITE = 0x0104;
/** The descriptor ended. A file opened for writing is replaced by what was written; an
 * open not yet answered is canceled, and never answered. */
export const SYS_CLOSE = 0x0105;
/** The broken invariant the guest reports as it stops (xpute-kit status/bug.rs). */
export const SYS_ABORT = 0x0106;
