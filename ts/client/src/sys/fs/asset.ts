// @pxl/client/sys/fs/asset.ts

/**
 * The backend of the core's ASSET root. The core names a file by the id of
 * its load, in decimal. The id `CATALOG` is the catalog, which names every
 * other file, and the client ships it in its own bundle. Any other id is a
 * row of the ingest's table, counted from 1, and is read from the origin by
 * its hash.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import { hex_to_bytes } from "@xpute/kit/codec/encoding.ts";
import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { FileError } from "@xpute/runtime";
import { CATALOG } from "@/abi/file.spec.ts";
import { CATALOG_HEX } from "@/sys/fs/catalog.spec.ts";
import { ASSET_FILES } from "@/origin/ingest.spec.ts";
import { asset_url } from "@/origin/route.ts";
import { get } from "@/sys/fs/read.ts";

export function read_asset(
  path: string,
  signal: AbortSignal,
): Promise<U8Array> {
  if (path === String(CATALOG)) {
    return Promise.resolve(hex_to_bytes(CATALOG_HEX));
  }
  const row = Number(path);
  const key = String(row) === path ? ASSET_FILES[row - 1]?.key : undefined;
  if (key === undefined) return Promise.reject(new FileError(Errno.ENOENT));
  return get(asset_url(key), signal);
}
