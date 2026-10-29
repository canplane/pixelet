// @pxl/client/namespace/namespace.ts

/**
 * The kernel's namespace: each number it opens a file under (host.json
 * `root`) bound to what fetches or keeps its bytes, a path below it an
 * object's number in decimal. The boot hands it to `@xpute/host`'s system
 * calls (abi/boot.ts).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { type Bound, Namespace } from "@xpute/host/ipc/ns.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { root } from "@/abi/host.spec.ts";
import { BASE_LAYERS } from "./published.spec.ts";
import { read_asset } from "@/namespace/asset.ts";
import { source_declared } from "@/namespace/source.ts";
import { commit_storage, open_storage } from "@/namespace/storage.ts";
import { read_object } from "@/namespace/tile.ts";

/** What answers below one number, by an object's number; one with no `commit` is read-only. */
interface Backend {
  open(number: bigint, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(number: bigint, bytes: U8Array): void;
}

/** A path's one name, an object's number in decimal; ENOENT for any other. */
function number_of(path: string): bigint {
  if (!/^\d{1,20}$/.test(path)) throw new FileError(Errno.ENOENT);
  return BigInt(path);
}

function by_number(backend: Backend): Bound {
  const { open, commit } = backend;
  return {
    open: (path, signal) => open(number_of(path), signal),
    commit: commit && ((path, bytes) => commit(number_of(path), bytes)),
  };
}

/** Each bake to its tiles at the edge, ASSET to the shipped assets, STORAGE to the platform's storage. */
export function namespace(): Namespace {
  const bound: [u32, Backend][] = [
    ...BASE_LAYERS.map((_, bake): [u32, Backend] => [
      bake,
      {
        open: (number, signal) =>
          read_object(source_declared(), bake, number, signal),
      },
    ]),
    [root.ASSET, { open: read_asset }],
    [root.STORAGE, { open: open_storage, commit: commit_storage }],
  ];
  return new Namespace(bound.map(([r, backend]) => [r, by_number(backend)]));
}
