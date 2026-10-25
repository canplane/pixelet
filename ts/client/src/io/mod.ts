// @pxl/client/io/mod.ts

/**
 * Which handler serves each number the kernel opens files under
 * (host.json `root`): tiles, assets and storage. A path is an object number
 * in decimal. The boot passes the table to `@xpute/host` (abi/boot.ts).
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { type Mounted, MountTable } from "@xpute/host/ipc/mount.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";

import { root } from "@/abi/host.spec.ts";
import { BASE_LAYERS } from "@/config/published.spec.ts";
import { read_asset } from "@/io/asset.ts";
import { source_declared } from "@/io/source.ts";
import { commit_storage, open_storage } from "@/io/storage.ts";
import { read_object } from "@/io/tile.ts";

/** Reads, and maybe writes, objects by number. */
interface Backend {
  open(number: bigint, signal: AbortSignal): U8Array | Promise<U8Array>;
  commit?(number: bigint, bytes: U8Array): void;
}

/** The object number a path names; ENOENT if it is not one. */
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

/** Each tile layer reads from the edge, ASSET from the shipped assets, STORAGE from the device. */
export function mount_table(): MountTable {
  const mounted: [u32, Backend][] = [
    ...BASE_LAYERS.map((_, bake): [u32, Backend] => [bake, { open: (number, signal) => read_object(source_declared(), bake, number, signal) }]),
    [root.ASSET, { open: read_asset }],
    [root.STORAGE, { open: open_storage, commit: commit_storage }],
  ];
  return new MountTable(mounted.map(([r, backend]) => [r, by_number(backend)]));
}
