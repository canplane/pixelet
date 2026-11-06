// @xpute/runtime/abi/fs.spec.ts
//
// GENERATED from spec/abi/fs.json

/**
 * The file device, on major FS: a core's files, opened below a root the runtime bound, as
 * POSIX's `openat` and io_uring's submission and completion name them (ipc/fs.json).
 * The core posts its calls (`import`), FS_OPENAT and FS_CLOSE with ACKREQ, tagged with
 * the descriptor; the runtime completes each of those on a later turn, a frame of the same
 * tag and command flagged RES on the submission ring, and does the rest in the gap.
 * xpute ships both ends: the core's calls (xpute-core ipc/door.rs, ipc/file.rs) and the
 * runtime's server (@xpute/runtime). A program binds only its roots to what backs them.
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./fs/major.spec.ts";
import * as FsMinor from "./fs/fs_minor.spec.ts";
import * as Command from "./fs/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { OnImport } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of FsMinor's, as its enum was a type. */
type FsMinor = (typeof FsMinor)[keyof typeof FsMinor];
export { FsMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.FS_OPENAT's completion's packet, written in place into `to`.
 *
 * @param detail Whatever number the runtime refused with — for HTTP its status — kept for a reader to print and for nothing to branch on. */
export function encode_fs_openat_reply(
  to: PacketRoom,
  detail: u32,
): PacketWriter {
  return to.writer(1).u32(detail);
}

/** Command.FS_OPENAT's packet. */
export interface FsOpenat {
  fd: u32;
  root: u32;
  /** ipc/fs.json `flag`. */
  flags: u32;
  path: string;
}

export function read_fs_openat(b: BranchCursor): FsOpenat {
  return {
    fd: b.at(0).get<u32>(),
    root: b.at(1).get<u32>(),
    flags: b.at(2).get<u32>(),
    path: b.at(3).get<string>(),
  };
}

/** Command.FS_READ's packet. */
export interface FsRead {
  fd: u32;
  at: u32;
  len: u32;
}

export function read_fs_read(b: BranchCursor): FsRead {
  return {
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  };
}

/** Command.FS_WRITE's packet. */
export interface FsWrite {
  fd: u32;
  at: u32;
  len: u32;
}

export function read_fs_write(b: BranchCursor): FsWrite {
  return {
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  };
}

/** Command.FS_CLOSE's packet. */
export interface FsClose {
  fd: u32;
}

export function read_fs_close(b: BranchCursor): FsClose {
  return { fd: b.at(0).get<u32>() };
}

/** What the program does with each command the core posts on the fs protocol; a
 * call's answer is its return. */
export interface FsImports {
  /** The file at `path` below `root` opened as `fd`, the frame's tag too. Completed with
   * the file's size, or -errno: ENOENT where there is no such file, ECANCELED where FS_CLOSE
   * came first, ENODEV where the machine has no file device. */
  fs_openat(e: FsOpenat): void;
  /** The opened file's first `len` bytes copied to `at` in the core's memory. */
  fs_read(e: FsRead): void;
  /** The `len` bytes at `at` added to what the file opened for writing holds. The runtime
   * reads them at FS_CLOSE, and the core keeps them unchanged until FS_CLOSE is completed. */
  fs_write(e: FsWrite): void;
  /** The descriptor ended, the frame's tag too. A file opened for writing is replaced by
   * what was written; an open not yet completed is completed first, with ECANCELED. Completed
   * with 0, or -errno where the file could not be replaced. */
  fs_close(e: FsClose): void;
}

/** `h` as the handler a runtime routes the fs protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function fs_imports(h: FsImports): OnImport {
  return (_tag, cmd, f) => {
    switch (cmd) {
      case Command.FS_OPENAT:
        return h.fs_openat(read_fs_openat(need(f)));
      case Command.FS_READ:
        return h.fs_read(read_fs_read(need(f)));
      case Command.FS_WRITE:
        return h.fs_write(read_fs_write(need(f)));
      case Command.FS_CLOSE:
        return h.fs_close(read_fs_close(need(f)));
      default:
        return Errno.ENOSYS;
    }
  };
}

/** A packet the command carries, which a frame without one breaks. */
function need<T>(f: T | null): T {
  if (f === null) throw new MarshalError(Errno.EBADMSG);
  return f;
}
