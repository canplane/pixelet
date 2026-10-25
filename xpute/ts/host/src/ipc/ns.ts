// @xpute/host/ipc/ns.ts

/**
 * The roots a program binds, Plan 9's namespace: each root the guest opens
 * a file under answered by whatever the program keeps there, bound once and
 * sealed. It hands `Sys` its opens and commits (sys.ts `SysHost`); what a
 * path below a root names is the bound side's alone.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError, type SysHost } from "./sys.ts";

/** What answers below one root; one with no `commit` is read-only. */
export interface Bound {
  open(path: string, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(path: string, bytes: U8Array): void;
}

export class Namespace implements Pick<SysHost, "open" | "commit"> {
  private readonly bound: ReadonlyMap<u32, Bound>;

  constructor(bound: Iterable<readonly [u32, Bound]>) {
    this.bound = new Map(bound);
  }

  /** The file at `path` below `root`, from what is bound there; ENOENT below a root none is. */
  readonly open = (root: u32, path: string, signal: AbortSignal): U8Array | Promise<U8Array> => {
    const bound = this.bound.get(root);
    if (bound === undefined) throw new FileError(Errno.ENOENT);
    return bound.open(path, signal);
  };

  /** `bytes` written to `path` below `root`; EROFS below one that cannot be written. */
  readonly commit = (root: u32, path: string, bytes: U8Array): void => {
    const write = this.bound.get(root)?.commit;
    if (write === undefined) throw new FileError(Errno.EROFS);
    write(path, bytes);
  };
}
