// @pxl/client/namespace/asset.ts

/**
 * ASSET's backend: the kernel names a shipped asset by its load number, and
 * this reads the asset that number is, by its hash, from the origin.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";
import { INGESTED } from "@/catalog/asset/ingested.spec.ts";
import { assetUrl } from "@/config/route.ts";
import { get } from "@/namespace/read.ts";

const KEYS: readonly string[] = Object.values(INGESTED).map((a) => a.key);

/** A load number: an asset's index in the ingest's table, the texture pack's among them. */
export function read_asset(load: bigint, signal: AbortSignal): Promise<U8Array> {
  const key = KEYS[Number(load)];
  if (key === undefined) return Promise.reject(new FileError(Errno.ENOENT));
  return get(assetUrl(key), key, signal);
}
