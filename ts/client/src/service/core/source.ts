// @pxl/client/service/core/source.ts

/** SOURCE on the UI's port: the part of the tile source's declaration that the
 * core is handed. What it says is sys/fs's to decide
 * (service/sys/fs/source.ts). */

import { core } from "@/service/core/core.ts";

/** Hands the core the top zoom, the formats and the shards (SOURCE_DECLARE).
 * Returns false when they are more than one command can carry, or when the
 * core refuses them. */
export function source_declare(
  min_zoom: number,
  formats: number[],
  shards: string[],
): boolean {
  return core.source_declare(min_zoom, formats, shards) >= 0;
}
