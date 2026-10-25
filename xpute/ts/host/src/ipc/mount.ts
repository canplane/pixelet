// @xpute/host/ipc/mount.ts

/**
 * The mount table. A guest opens a file by a number and a path. The program
 * mounts one handler on each number, once, when it starts. Opening a file
 * calls the handler mounted on that number. No handler means ENOENT. A
 * handler with no `commit` is read-only, so writing to it means EROFS.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError, type SysHost } from "./sys.ts";

/** Reads, and maybe writes, the files under one number. */
export interface Mounted {
  open(path: string, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(path: string, bytes: U8Array): void;
}

export class MountTable implements Pick<SysHost, "open" | "commit"> {
  private readonly mounted: ReadonlyMap<u32, Mounted>;

  constructor(mounted: Iterable<readonly [u32, Mounted]>) {
    this.mounted = new Map(mounted);
  }

  readonly open = (root: u32, path: string, signal: AbortSignal): U8Array | Promise<U8Array> => {
    const mounted = this.mounted.get(root);
    if (mounted === undefined) throw new FileError(Errno.ENOENT);
    return mounted.open(path, signal);
  };

  readonly commit = (root: u32, path: string, bytes: U8Array): void => {
    const write = this.mounted.get(root)?.commit;
    if (write === undefined) throw new FileError(Errno.EROFS);
    write(path, bytes);
  };
}
