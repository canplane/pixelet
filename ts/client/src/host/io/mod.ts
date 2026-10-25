// @pxl/client/host/io/mod.ts

/**
 * The mount table of the kernel: each number it opens a file under
 * (host.json `root`) mounted on what fetches or keeps its bytes, a path
 * below it the number of an object in decimal. The boot hands it to the
 * system calls of `@xpute/host` (abi/boot.ts).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { type Mounted, MountTable } from "@xpute/host/ipc/mount.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { root } from "@/host/abi/host.spec.ts";
import { BASE_LAYERS } from "@/host/config/published.spec.ts";
import { read_asset } from "@/host/io/asset.ts";
import { source_declared } from "@/host/io/source.ts";
import { commit_storage, open_storage } from "@/host/io/storage.ts";
import { read_object } from "@/host/io/tile.ts";

/** What answers below one number, by the number of an object; one with no `commit` is read-only. */
interface Backend {
  open(number: bigint, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(number: bigint, bytes: U8Array): void;
}

/** The one name of a path, the number of an object in decimal; ENOENT for any other. */
function number_of(path: string): bigint {
  if (!/^\d{1,20}$/.test(path)) throw new FileError(Errno.ENOENT);
  return BigInt(path);
}

function by_number(backend: Backend): Mounted {
  const { open, commit } = backend;
  return {
    open: (path, signal) => open(number_of(path), signal),
    commit: commit && ((path, bytes) => commit(number_of(path), bytes)),
  };
}

/** Each bake to its tiles at the edge, ASSET to the shipped assets, STORAGE to the storage of the platform. */
export function mount_table(): MountTable {
  const mounted: [u32, Backend][] = [
    ...BASE_LAYERS.map((_, bake): [u32, Backend] => [bake, { open: (number, signal) => read_object(source_declared(), bake, number, signal) }]),
    [root.ASSET, { open: read_asset }],
    [root.STORAGE, { open: open_storage, commit: commit_storage }],
  ];
  return new MountTable(mounted.map(([r, backend]) => [r, by_number(backend)]));
}
