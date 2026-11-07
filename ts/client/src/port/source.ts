// @pxl/client/port/source.ts

/** SOURCE on the shell's port: the tile source's part the kernel is handed.
 * What it says is sys/fs's to decide (source.ts). */

import { kernel } from "@/runtime/kernel.ts";

/** The top zoom, the formats and the shards (SOURCE_DECLARE); false where
 * they are more than a command carries or the kernel refuses them. */
export function source_declare(
  top_zoom: number,
  formats: number[],
  shards: string[],
): boolean {
  return kernel.source_declare(top_zoom, formats, shards) >= 0;
}
