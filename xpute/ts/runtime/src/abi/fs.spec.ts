// @xpute/runtime/abi/fs.spec.ts
//
// GENERATED from spec/abi/fs.json

/**
 * The file device's protocol on major FS: a core's files, each opened below a root the
 * runtime has bound, in the terms of POSIX's `openat` and of io_uring's submissions and
 * completions (ipc/fs.json). The core posts its calls as imports. It posts FS_OPENAT and
 * FS_CLOSE with ACKREQ, tagged with the descriptor, and the runtime completes each of them
 * on a later turn with a frame of the same tag and command, flagged RES, on the submission
 * ring. The runtime does the rest of the work in the gap between turns. It tells the core
 * that a file changed under it with FS_CHANGED, an export. xpute ships both ends: the core's
 * calls (xpute-core ipc/door.rs, ipc/file.rs) and the runtime's server (@xpute/runtime). A
 * program only binds its roots to what backs them.
 */

import type { BranchCursor } from "@xpute/kit/wire/xtp/mod.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Major from "./fs/major.spec.ts";
import * as FsMinor from "./fs/fs_minor.spec.ts";
import * as Command from "./fs/command.spec.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { MarshalError } from "@xpute/kit/status/error.ts";
import type { Protocol, OnImport } from "../mod.ts";

/** One of Major's, as its enum was a type. */
type Major = (typeof Major)[keyof typeof Major];
export { Major };

/** One of FsMinor's, as its enum was a type. */
type FsMinor = (typeof FsMinor)[keyof typeof FsMinor];
export { FsMinor };

/** One of Command's, as its enum was a type. */
type Command = (typeof Command)[keyof typeof Command];
export { Command };

/** Command.FS_OPENAT's packet. */
export interface FsOpenat {
  fd: u32;
  root: u32;
  /** One of the `flag` values of ipc/fs.json. */
  flags: u32;
  path: string;
}

/** Command.FS_READ's packet. */
export interface FsRead {
  fd: u32;
  at: u32;
  len: u32;
}

/** Command.FS_WRITE's packet. */
export interface FsWrite {
  fd: u32;
  at: u32;
  len: u32;
}

/** Command.FS_CLOSE's packet. */
export interface FsClose {
  fd: u32;
}

/** Each import's packet, read from its branch, by its number. */
export const read = {
  [Command.FS_OPENAT]: (b: BranchCursor): FsOpenat => ({
    fd: b.at(0).get<u32>(),
    root: b.at(1).get<u32>(),
    flags: b.at(2).get<u32>(),
    path: b.at(3).get<string>(),
  }),
  [Command.FS_READ]: (b: BranchCursor): FsRead => ({
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  }),
  [Command.FS_WRITE]: (b: BranchCursor): FsWrite => ({
    fd: b.at(0).get<u32>(),
    at: b.at(1).get<u32>(),
    len: b.at(2).get<u32>(),
  }),
  [Command.FS_CLOSE]: (b: BranchCursor): FsClose => ({
    fd: b.at(0).get<u32>(),
  }),
};

/** What the runtime sends on the fs protocol: each command by its number, and
 * the fields its packet takes in order. It is sent, requested and answered
 * later, or called and answered before it returns. */
export type FsSend = {
  /** @param path The file's path below the root, spelled as the core opened it. */
  [Command.FS_CHANGED]: [root: u32, path: string];
};
export type FsRequest = Record<never, never>;
export type FsCall = Record<never, never>;

/** The fs protocol over a link. */
export type FsProtocol = Protocol<FsSend, FsRequest, FsCall>;

/** What the program does with each command the core posts on the fs protocol; a
 * call's answer is its return. */
export interface FsImports {
  /** Opens the file at `path` below `root` as descriptor `fd`, which is also the frame's tag.
   * The runtime completes it with the file's size, or with -errno: ENOENT when there is no
   * such file, ECANCELED when FS_CLOSE came first, and ENODEV when the machine has no file
   * device. */
  [Command.FS_OPENAT](e: FsOpenat): void;
  /** Copies the first `len` bytes of the opened file to `at` in the core's memory. */
  [Command.FS_READ](e: FsRead): void;
  /** Appends the `len` bytes at `at` to what the file, opened for writing, will hold. The
   * runtime reads the bytes at FS_CLOSE, so the core keeps them unchanged until FS_CLOSE is
   * completed. */
  [Command.FS_WRITE](e: FsWrite): void;
  /** Ends the descriptor, which is also the frame's tag. A file opened for writing is replaced
   * by what was written to it. An open not yet completed is completed first, with ECANCELED.
   * The close is completed with 0, or with -errno when the file could not be replaced. */
  [Command.FS_CLOSE](e: FsClose): void;
}

/** `h` as the handler a runtime routes the fs protocol's commands to, each one read and
 * handed to its member; ENOSYS for one the protocol does not carry. */
export function fs_imports(h: FsImports): OnImport {
  return (_tag, cmd, f) => {
    switch (cmd) {
      case Command.FS_OPENAT:
        return h[Command.FS_OPENAT](read[Command.FS_OPENAT](need(f)));
      case Command.FS_READ:
        return h[Command.FS_READ](read[Command.FS_READ](need(f)));
      case Command.FS_WRITE:
        return h[Command.FS_WRITE](read[Command.FS_WRITE](need(f)));
      case Command.FS_CLOSE:
        return h[Command.FS_CLOSE](read[Command.FS_CLOSE](need(f)));
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
