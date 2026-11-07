// @pxl/client/port/source.ts

/** SOURCE on the UI's port: the tile source's part the core is handed.
 * What it says is sys/fs's to decide (source.ts). */

import { core } from "@/port/core.ts";

/** The top zoom, the formats and the shards (SOURCE_DECLARE); false where
 * they are more than a command carries or the core refuses them. */
export function source_declare(
  top_zoom: number,
  formats: number[],
  shards: string[],
): boolean {
  return core.source_declare(top_zoom, formats, shards) >= 0;
}
