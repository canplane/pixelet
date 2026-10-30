// @xpute/host/ipc/ns.ts

/**
 * The roots a program binds, Plan 9's namespace, as the `Fs` it hands `Sys`
 * (sys.ts): each root the guest opens a file under answered by whatever the
 * program keeps there, bound once and sealed. What a path below a root names
 * is the bound side's alone.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError, type Fs } from "./sys.ts";

/** What answers below one root; one with no `commit` is read-only. */
export interface Bound {
  open(path: string, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(path: string, bytes: U8Array): void;
}

export class Namespace implements Fs {
  private readonly bound: ReadonlyMap<u32, Bound>;

  constructor(bound: Iterable<readonly [u32, Bound]>) {
    this.bound = new Map(bound);
  }

  /** ENOENT below a root nothing is bound to. */
  open(
    root: u32,
    path: string,
    signal: AbortSignal,
  ): U8Array | Promise<U8Array> {
    const bound = this.bound.get(root);
    if (bound === undefined) throw new FileError(Errno.ENOENT);
    return bound.open(path, signal);
  }

  /** EROFS below a root that cannot be written. */
  commit(root: u32, path: string, bytes: U8Array): void {
    const write = this.bound.get(root)?.commit;
    if (write === undefined) throw new FileError(Errno.EROFS);
    write(path, bytes);
  }
}
