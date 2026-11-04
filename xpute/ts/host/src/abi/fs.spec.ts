// @xpute/host/abi/fs.spec.ts
//
// GENERATED from spec/abi/fs.json

/**
 * The file device, on major FS: a guest's files, opened below a root the host bound, as
 * POSIX's `openat` and io_uring's submission and completion name them (ipc/fs.json).
 * The guest posts its calls (`import`); the host answers an open with FS_OPENED (`export`) on a
 * later turn, and does the rest in the gap. xpute ships both ends: the guest's calls
 * (xpute-guest ipc/door.rs, ipc/file.rs) and the host's server (@xpute/host). A program
 * binds only its roots to what backs them.
 */

import type {
  BranchCursor,
  PacketRoom,
  PacketWriter,
} from "@xpute/kit/wire/xtp/mod.ts";
import type { i32, u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./fs/major.spec.ts";
import * as FsMinor from "./fs/fs_minor.spec.ts";
import * as Command from "./fs/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Host, OnEvent } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of FsMinor's, as its enum was a type. */
type FsMinor = (typeof FsMinor)[keyof typeof FsMinor];
export { FsMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.FS_OPENED's packet, written in place into `to`.
 *
 * @param detail Whatever number the host refused with — for HTTP its status — kept for a reader to print and for nothing to branch on. */
export function encode_fs_opened(
  to: PacketRoom,
  fd: u32,
  res: i32,
  detail: u32,
): PacketWriter {
  return to.writer(3).u32(fd).i32(res).u32(detail);
}

/** Command.FS_OPENAT's event. */
export interface FsOpenatEvent {
  fd: u32;
  root: u32;
  /** ipc/fs.json `flag`. */
  flags: u32;
  path: string;
}

export function read_fs_openat(b: BranchCursor): FsOpenatEvent {
  return {
    fd: b.at(0).get<u32>(),
    root: b.at(1).get<u32>(),
    flags: b.at(2).get<u32>(),
    path: b.at(3).get<string>(),
  };
}

/** Command.FS_READ's event. */
export interface FsReadEvent {
  fd: u32;
  at: u32;
  len: u32;
}

export function read_fs_read(b: BranchCursor): FsReadEvent {
  return {
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  };
}

/** Command.FS_WRITE's event. */
export interface FsWriteEvent {
  fd: u32;
  at: u32;
  len: u32;
}

export function read_fs_write(b: BranchCursor): FsWriteEvent {
  return {
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  };
}

/** Command.FS_CLOSE's event. */
export interface FsCloseEvent {
  fd: u32;
}

export function read_fs_close(b: BranchCursor): FsCloseEvent {
  return { fd: b.at(0).get<u32>() };
}

/** What the host sends on the fs protocol, bound to `host`: each
 * command rung, or called and answered before it returns. */
export function fs_exports(host: Host) {
  return {
    fs_opened: (fd: u32, res: i32, detail: u32): void =>
      host.send(Command.FS_OPENED, (r) => encode_fs_opened(r, fd, res, detail)),
  };
}

/** The fs protocol's stubs, bound. */
export type FsExports = ReturnType<typeof fs_exports>;

/** What the program does with each command the guest posts on the fs protocol; a
 * call's answer is its return. */
export interface FsImports {
  /** The file at `path` below `root` opened as `fd`, answered by FS_OPENED. */
  fs_openat(e: FsOpenatEvent): void;
  /** The opened file's first `len` bytes copied to `at` in the guest's memory. */
  fs_read(e: FsReadEvent): void;
  /** The `len` bytes at `at` added to what the file opened for writing holds. */
  fs_write(e: FsWriteEvent): void;
  /** The descriptor ended. A file opened for writing is replaced by what was written; an
   * open not yet answered is canceled, and never answered. */
  fs_close(e: FsCloseEvent): void;
}

/** `h` as the handler a host routes the fs protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function fs_imports(h: FsImports): OnEvent {
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
