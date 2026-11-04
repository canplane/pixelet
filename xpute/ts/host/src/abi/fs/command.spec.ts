// @xpute/host/abi/fs/command.spec.ts
//
// GENERATED from spec/abi/fs.json

// Every command, by the side that sends it.

// ---- fs, export ----

/** An FS_OPENAT answered: the file's size, or -errno — ENOENT where there is no such file.
 * Never sent for a descriptor already closed. */
export const FS_OPENED = 0x0301;

// ---- fs, import ----

/** The file at `path` below `root` opened as `fd`, answered by FS_OPENED. */
export const FS_OPENAT = 0x0302;
/** The opened file's first `len` bytes copied to `at` in the guest's memory. */
export const FS_READ = 0x0303;
/** The `len` bytes at `at` added to what the file opened for writing holds. */
export const FS_WRITE = 0x0304;
/** The descriptor ended. A file opened for writing is replaced by what was written; an
 * open not yet answered is canceled, and never answered. */
export const FS_CLOSE = 0x0305;
