// @pxl/client/host/io/roots.ts

/**
 * The guest's namespace: each root it opens files below (host.json `root`)
 * bound to the backend that answers it, as Plan 9 binds a name. Bound once,
 * at boot, and sealed: no root is bound after. `@xpute/host` hands every
 * open to the host's one opener (sys.ts), which dispatches here by root.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { root } from "@/host/abi/host.spec.ts";
import { BASE_LAYERS } from "@/config/published.spec.ts";
import { read_asset } from "@/host/io/asset.ts";
import { source_declared } from "@/host/io/source.ts";
import { open_storage } from "@/host/io/storage.ts";
import { read_object } from "@/host/io/tile.ts";

/** What a root's object is, by its number: every root is named so (host.json `root`). */
type Open = (number: bigint, signal: AbortSignal) => U8Array | Promise<U8Array>;

/** Each root's backend; none until the boot binds them. */
let ROOTS: ReadonlyMap<u32, Open> | null = null;

/** Binds every root to its backend, once: each bake to its tiles at the edge,
 * ASSET to the shipped assets, STORAGE to the platform's storage. */
export function bind_roots(): void {
  if (ROOTS !== null) throw new Error("roots: bound twice, after the seal");
  ROOTS = new Map<u32, Open>([
    ...BASE_LAYERS.map((_, bake): [u32, Open] => [bake, (number, signal) => read_object(source_declared(), bake, number, signal)]),
    [root.ASSET, read_asset],
    [root.STORAGE, open_storage],
  ]);
}

/** A path's one name, an object's number in decimal; ENOENT for any other. */
export function number_of(path: string): bigint {
  if (!/^\d{1,20}$/.test(path)) throw new FileError(Errno.ENOENT);
  return BigInt(path);
}

/** The file at `path` below root `r`, from the backend bound there; ENOENT below a root none is. */
export function open_root(r: u32, path: string, signal: AbortSignal): U8Array | Promise<U8Array> {
  const bound = ROOTS?.get(r);
  if (bound === undefined) throw new FileError(Errno.ENOENT);
  return bound(number_of(path), signal);
}
