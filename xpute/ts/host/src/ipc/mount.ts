// @xpute/host/ipc/mount.ts

/**
 * The mount table: what answers below each number a guest opens a file
 * under, mounted once and sealed. It hands `Sys` its opens and commits
 * (sys.ts `SysHost`); what a path below a mount names is up to what is
 * mounted there alone.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError, type SysHost } from "./sys.ts";

/** What answers below one root; one with no `commit` is read-only. */
export interface Mounted {
  open(path: string, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(path: string, bytes: U8Array): void;
}

export class MountTable implements Pick<SysHost, "open" | "commit"> {
  private readonly mounted: ReadonlyMap<u32, Mounted>;

  constructor(mounted: Iterable<readonly [u32, Mounted]>) {
    this.mounted = new Map(mounted);
  }

  /** The file at `path` below `root`, from what is mounted there; ENOENT where nothing is. */
  readonly open = (root: u32, path: string, signal: AbortSignal): U8Array | Promise<U8Array> => {
    const mounted = this.mounted.get(root);
    if (mounted === undefined) throw new FileError(Errno.ENOENT);
    return mounted.open(path, signal);
  };

  /** `bytes` written to `path` below `root`; EROFS where what is mounted cannot be written. */
  readonly commit = (root: u32, path: string, bytes: U8Array): void => {
    const write = this.mounted.get(root)?.commit;
    if (write === undefined) throw new FileError(Errno.EROFS);
    write(path, bytes);
  };
}
