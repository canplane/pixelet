// @xpute/runtime/abi/fs/command.spec.ts
//
// GENERATED from spec/abi/fs.json

// Every command, by the side that sends it.

// ---- fs, import ----

/** The file at `path` below `root` opened as `fd`, the frame's tag too. Completed with
 * the file's size, or -errno: ENOENT where there is no such file, ECANCELED where FS_CLOSE
 * came first, ENODEV where the machine has no file device. */
export const FS_OPENAT = 0x0302;
/** The opened file's first `len` bytes copied to `at` in the core's memory. */
export const FS_READ = 0x0303;
/** The `len` bytes at `at` added to what the file opened for writing holds. The runtime
 * reads them at FS_CLOSE, and the core keeps them unchanged until FS_CLOSE is completed. */
export const FS_WRITE = 0x0304;
/** The descriptor ended, the frame's tag too. A file opened for writing is replaced by
 * what was written; an open not yet completed is completed first, with ECANCELED. Completed
 * with 0, or -errno where the file could not be replaced. */
export const FS_CLOSE = 0x0305;
