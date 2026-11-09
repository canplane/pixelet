// @xpute/runtime/ipc/ns.ts

/**
 * These are the roots a program binds, after Plan 9's namespace, as the `Fs` it
 * hands `FsServer` (fs.ts). Each root the core opens files under is answered by
 * whatever the program binds there. The bindings are made once, when the
 * namespace is built, and never change. What a path below a root names is for
 * the bound side alone to decide.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { type Fetcher, FileError, type Fs } from "./fs.ts";

/** A binding answers below one root, and one with no `commit` is read-only. */
export interface Binding {
  open(path: string, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(path: string, bytes: U8Array): void;
  /** Where the bytes of `path` can be fetched from. */
  url?(path: string): string;
}

export class Namespace implements Fs {
  private readonly bound: ReadonlyMap<u32, Binding>;
  readonly fetcher: Fetcher | undefined;

  constructor(bound: Iterable<readonly [u32, Binding]>, fetcher?: Fetcher) {
    this.bound = new Map(bound);
    this.fetcher = fetcher;
  }

  url(root: u32, path: string): string | undefined {
    return this.bound.get(root)?.url?.(path);
  }

  /** This throws ENOENT below a root nothing is bound to. */
  open(
    root: u32,
    path: string,
    signal: AbortSignal,
  ): U8Array | Promise<U8Array> {
    const bound = this.bound.get(root);
    if (bound === undefined) throw new FileError(Errno.ENOENT);
    return bound.open(path, signal);
  }

  /** This throws EROFS below a root that cannot be written. */
  commit(root: u32, path: string, bytes: U8Array): void {
    const write = this.bound.get(root)?.commit;
    if (write === undefined) throw new FileError(Errno.EROFS);
    write(path, bytes);
  }
}
