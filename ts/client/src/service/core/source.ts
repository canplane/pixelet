// @pxl/client/service/core/source.ts

import { core } from "@/service/core/core.ts";

/** Resolves false when this part of the declaration is more than one command
 * can carry, or when the core refuses it. */
export async function source_declare(
  min_zoom: number,
  formats: number[],
  shards: string[],
): Promise<boolean> {
  try {
    await core.source_declare(min_zoom, Uint32Array.from(formats), shards);
    return true;
  } catch {
    return false;
  }
}
