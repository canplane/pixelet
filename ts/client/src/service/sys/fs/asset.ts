// @pxl/client/service/sys/fs/asset.ts

/**
 * The backend of the core's ASSET files. The core names a file by the id of
 * its load (ASSET, id). The id `CATALOG` is the catalog, which names every
 * other file, and the client ships it in its own bundle. Any other id is a
 * row of the ingest's table, counted from 1, and is read from the origin by
 * its hash.
 */

import type { U8Array } from "@xpute/kit/abi/array.ts";
import type { u32 } from "@xpute/kit/abi/word.ts";
import { hex_to_bytes } from "@xpute/kit/codec/encoding.ts";
import { ComponentError } from "@xpute/link";
import { ERROR_CODE_NO_ENTRY } from "@xpute/host/abi/fs/types.enum.ts";
import { CATALOG } from "@/abi/file.spec.ts";
import { CATALOG_HEX } from "@/service/sys/fs/catalog.spec.ts";
import { ASSET_FILES } from "@/service/origin/ingest.spec.ts";
import { assetUrl } from "@/service/origin/route.ts";
import { get } from "@/service/sys/fs/read.ts";

export function readAsset(id: u32, signal: AbortSignal): Promise<U8Array> {
  if (id === CATALOG) return Promise.resolve(hex_to_bytes(CATALOG_HEX));
  const key = ASSET_FILES[id - 1]?.key;
  if (key === undefined)
    return Promise.reject(new ComponentError(ERROR_CODE_NO_ENTRY));
  return get(assetUrl(key), signal);
}
