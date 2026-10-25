// @pxl/client/host/io/asset.ts

/**
 * ASSET's backend: the kernel names a file by its load number, and this reads
 * what that number is. `CATALOG` is the catalog, which the host plugs in from
 * its own bundle and which names every other file; any other number is a row
 * of the ingest's table, counted from 1, read by its hash from the origin.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { hex_to_bytes } from "@xpute/kit/codec/encoding.ts";
import { Errno } from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";
import { CATALOG } from "@/host/abi/host.spec.ts";
import { CATALOG_HEX } from "@/host/io/catalog.spec.ts";
import { INGESTED } from "@/host/io/ingested.spec.ts";
import { asset_url } from "@/host/config/route.ts";
import { get } from "@/host/io/read.ts";

const KEYS: readonly string[] = Object.values(INGESTED).map((a) => a.key);

/** A load number: the catalog's, or a file's row in the ingest's table from 1. */
export function read_asset(load: bigint, signal: AbortSignal): Promise<U8Array> {
  if (load === BigInt(CATALOG)) return Promise.resolve(hex_to_bytes(CATALOG_HEX));
  const key = KEYS[Number(load) - 1];
  if (key === undefined) return Promise.reject(new FileError(Errno.ENOENT));
  return get(asset_url(key), key, signal);
}
