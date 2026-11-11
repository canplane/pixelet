// @xpute/runtime/abi/fs.spec.ts
//
// GENERATED from spec/abi/fs.json

/**
 * The file device's protocol on major FS: a core's files, read whole into its own buffers
 * and written whole, through a descriptor, as WASI's path_open, fd_read, fd_write and
 * fd_close do. A file's name is an absolute path, a tuple of tokens, each an i32
 * or a string, as many as the program uses. xpute gives no token a meaning, and the
 * program's one binding answers every path (@xpute/runtime io/fs.ts).
 *
 * The core sends its calls as imports. The core issues each descriptor, a generational
 * handle, and every frame of a descriptor carries it once, as its tag. FS_OPEN and FS_CLOSE
 * go with ACKREQ, and the runtime completes each once, on a later turn, with a frame of the
 * same tag and command, flagged RES. It serves a descriptor's frames in the order they came,
 * so an FS_READ is done before its close is completed, and the memory an FS_READ names is the
 * runtime's until then. An FS_WRITE's bytes go with its frame, and the file is replaced by
 * them at FS_CLOSE. The runtime tells the core that a file changed under it with
 * FS_CHANGED, an export. xpute ships both ends: the core's calls (xpute-core io/door.rs,
 * io/file.rs) and the runtime's server (@xpute/runtime). A program only gives the binding
 * that backs its paths.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { INT_U32 } from "./number.ts";
import type { Path } from "../io/path.ts";
import { read_path } from "../io/path.ts";
import * as Major from "./fs/major.spec.ts";
import * as FsMinor from "./fs/fs_minor.spec.ts";
import * as Command from "./fs/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { Branch } from "@xpute/kit/wire/xtp/mod.ts";
import type { U8Array } from "@xpute/kit/abi/array.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Link, Protocol, OnImport } from "../mod.ts";
import { refusal_of } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of FsMinor's, as its enum was a type. */
type FsMinor = (typeof FsMinor)[keyof typeof FsMinor];
export { FsMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.FS_OPEN's packet. */
export interface FsOpen {
  path: Path;
  /** One of the `flag` values of io/fs.json. */
  flags: u32;
}

/** Command.FS_READ's packet. */
export interface FsRead {
  at: u32;
  len: u32;
}

/** Command.FS_WRITE's packet. */
export interface FsWrite {
  at: u32;
  len: u32;
}

/** Command.FS_CLOSE's packet. */
export interface FsClose {}

/** Each import's packet, read from its branch, by its number. */
export const read = {
  [Command.FS_OPEN]: (b: BranchCursor): FsOpen => ({
    path: read_path(b.at_branch(0)),
    flags: INT_U32(b.at(1).get_number()),
  }),
  [Command.FS_READ]: (b: BranchCursor): FsRead => ({
    at: INT_U32(b.at(0).get_number()),
    len: INT_U32(b.at(1).get_number()),
  }),
  [Command.FS_WRITE]: (b: BranchCursor): FsWrite => ({
    at: INT_U32(b.at(0).get_number()),
    len: INT_U32(b.at(1).get_number()),
  }),
};

/** The fs protocol's exports as the UI calls them over `link`: a send returns
 * nothing, and a request a Promise of its reply's value, or of the memory its reply
 * names, which rejects with the errno it was refused with. `fits` says whether a
 * send's packet fits a frame. */
export function fs_exports(link: Link) {
  return {
    /** The file at `path` changed under the core, as reported by what backs it. What the core
     * has read of the file is stale, and to read it again the core opens it again. */
    fs_changed: (path: Branch) => link.send(Command.FS_CHANGED, path),
    fits: {
      fs_changed: (path: Branch) => link.fits(Command.FS_CHANGED, path),
    },
  };
}

/** The fs protocol's exports, as `fs_exports` binds them. */
export type FsExports = ReturnType<typeof fs_exports>;

/** The majors the runtime routes the protocol's imports by. */
export const MAJORS: readonly u32[] = [Major.FS];

/** What the program does with each command the core posts on the fs protocol: a
 * call's member answers its value, and throws kit's error to refuse it. */
export interface FsImports {
  /** Opens the file at `path` as the descriptor the frame's tag carries, as WASI's path_open
   * does. The runtime completes an open to read once the file can be read whole, with its size,
   * and an open to write with 0; or either with -errno: ENOENT when there is no such file to
   * read, EINVAL when `flags` is not one of io/fs.json's `flag`, which the descriptor's close
   * then completes with too, ECANCELED when FS_CLOSE came first, and ENODEV when the manifest has
   * no file device. */
  [Command.FS_OPEN](fd: u32, e: FsOpen): void;
  /** Copies the first `len` bytes of the opened file to `at` in the core's memory, as WASI's
   * fd_read reads into a caller's buffer. A read of a file opened to write, or of one whose
   * open has not completed, copies nothing, and the descriptor's close completes with EBADF.
   * Where the manifest has no file device, it is refused with ENODEV. */
  [Command.FS_READ](fd: u32, e: FsRead): void;
  /** Appends the `len` bytes at `at` to what the file, opened to write, will hold, as WASI's
   * fd_write does. The bytes go with the frame, so the core keeps nothing. Nothing completes
   * it: the file's close says whether what was written replaced it. A write to a file opened
   * to read keeps nothing, and the descriptor's close completes with EBADF.
   * Where the manifest has no file device, it is refused with ENODEV. */
  [Command.FS_WRITE](fd: u32, e: FsWrite, named: U8Array): void;
  /** Ends the descriptor, as WASI's fd_close does. A file opened to write is replaced by what
   * was written to it, handed to the program's binding whole. An open not yet completed is
   * completed first, with ECANCELED. The close is completed with: 0; the errno the binding
   * refused the written file with; EINVAL where its open was refused for its flags; EBADF for a
   * descriptor the runtime does not hold, or one read while opened to write or before its open
   * completed, or written while opened to read; EFAULT where a read of it named memory outside
   * the core's or at offset 0; ENODEV where the manifest has no file device; or ECANCELED where a
   * reload of the program left the file unserved. */
  [Command.FS_CLOSE](fd: u32): void;
}

/** `h` as the protocol the runtime routes the fs protocol's majors to: each
 * command read and handed to its member, whose refusal is answered with its
 * errno; ENOSYS for one the protocol does not carry. */
export function fs_imports(h: FsImports): Protocol {
  const on: OnImport = (tag, cmd, f, _pkt, named) => {
    try {
      switch (cmd) {
        case Command.FS_OPEN:
          return h[Command.FS_OPEN](tag, read[Command.FS_OPEN](need(f)));
        case Command.FS_READ:
          return h[Command.FS_READ](tag, read[Command.FS_READ](need(f)));
        case Command.FS_WRITE:
          return h[Command.FS_WRITE](
            tag,
            read[Command.FS_WRITE](need(f)),
            need(named),
          );
        case Command.FS_CLOSE:
          return h[Command.FS_CLOSE](tag);
        default:
          return Errno.ENOSYS;
      }
    } catch (err) {
      return refusal_of(err);
    }
  };
  return { majors: MAJORS, on };
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
