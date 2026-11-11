// @xpute/runtime/abi/fs/command.spec.ts
//
// GENERATED from spec/abi/fs.json

// Every command, by the side that sends it.

// ---- fs, export ----

/** The file at `path` changed under the core, as reported by what backs it. What the core
 * has read of the file is stale, and to read it again the core opens it again. */
export const FS_CHANGED = 0x0305;

// ---- fs, import ----

/** Opens the file at `path` as the descriptor the frame's tag carries, as WASI's path_open
 * does. The runtime completes an open to read once the file can be read whole, with its size,
 * and an open to write with 0; or either with -errno: ENOENT when there is no such file to
 * read, EINVAL when `flags` is not one of io/fs.json's `flag`, which the descriptor's close
 * then completes with too, ECANCELED when FS_CLOSE came first, and ENODEV when the manifest has
 * no file device. */
export const FS_OPEN = 0x0301;
/** Copies the first `len` bytes of the opened file to `at` in the core's memory, as WASI's
 * fd_read reads into a caller's buffer. A read of a file opened to write, or of one whose
 * open has not completed, copies nothing, and the descriptor's close completes with EBADF.
 * Where the manifest has no file device, it is refused with ENODEV. */
export const FS_READ = 0x0302;
/** Appends the `len` bytes at `at` to what the file, opened to write, will hold, as WASI's
 * fd_write does. The bytes go with the frame, so the core keeps nothing. Nothing completes
 * it: the file's close says whether what was written replaced it. A write to a file opened
 * to read keeps nothing, and the descriptor's close completes with EBADF.
 * Where the manifest has no file device, it is refused with ENODEV. */
export const FS_WRITE = 0x0303;
/** Ends the descriptor, as WASI's fd_close does. A file opened to write is replaced by what
 * was written to it, handed to the program's binding whole. An open not yet completed is
 * completed first, with ECANCELED. The close is completed with: 0; the errno the binding
 * refused the written file with; EINVAL where its open was refused for its flags; EBADF for a
 * descriptor the runtime does not hold, or one read while opened to write or before its open
 * completed, or written while opened to read; EFAULT where a read of it named memory outside
 * the core's or at offset 0; ENODEV where the manifest has no file device; or ECANCELED where a
 * reload of the program left the file unserved. */
export const FS_CLOSE = 0x0304;
