// @xpute/runtime/abi/fs/command.spec.ts
//
// GENERATED from spec/abi/fs.json

// Every command, by the side that sends it.

// ---- fs, export ----

/** The file at `path` below `root` changed under the core, as reported by what backs the
 * root. What the core has read of the file is stale, and to read it again the core opens
 * it again. */
export const FS_CHANGED = 0x0305;

// ---- fs, import ----

/** Opens the file at `path` below `root` as descriptor `fd`, which is also the frame's tag.
 * The runtime completes it with the file's size, or with -errno: ENOENT when there is no
 * such file, ECANCELED when FS_CLOSE came first, and ENODEV when the machine has no file
 * device. */
export const FS_OPENAT = 0x0301;
/** Copies the first `len` bytes of the opened file to `at` in the core's memory. */
export const FS_READ = 0x0302;
/** Appends the `len` bytes at `at` to what the file, opened for writing, will hold. The
 * runtime reads the bytes at FS_CLOSE, so the core keeps them unchanged until FS_CLOSE is
 * completed. */
export const FS_WRITE = 0x0303;
/** Ends the descriptor, which is also the frame's tag. A file opened for writing is replaced
 * by what was written to it. An open not yet completed is completed first, with ECANCELED.
 * The close is completed with 0, or with -errno when the file could not be replaced. */
export const FS_CLOSE = 0x0304;
