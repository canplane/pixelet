// @pxl/client/driver/namespace/asset.ts

/**
 * ASSET's backend: the kernel names a file by its load's id, and this reads
 * what that is. `CATALOG` is the catalog, which the client plugs in from
 * its own bundle and which names every other file; any other id is a row
 * of the ingest's table, counted from 1, read by its hash from the origin.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { hex_to_bytes } from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/host/ipc/sys.ts";
import { CATALOG } from "@/abi/file.spec.ts";
import { CATALOG_HEX } from "@/driver/namespace/catalog.spec.ts";
import { ASSET_FILES } from "@/driver/namespace/ingest.spec.ts";
import { asset_url } from "@/driver/namespace/route.ts";
import { get } from "@/driver/namespace/read.ts";

/** A load's id: the catalog's, or a file's row in the ingest's table from 1. */
export function read_asset(
  load: bigint,
  signal: AbortSignal,
): Promise<U8Array> {
  if (load === BigInt(CATALOG)) {
    return Promise.resolve(hex_to_bytes(CATALOG_HEX));
  }
  const key = ASSET_FILES[Number(load) - 1]?.key;
  if (key === undefined) return Promise.reject(new FileError(Errno.ENOENT));
  return get(asset_url(key), key, signal);
}
