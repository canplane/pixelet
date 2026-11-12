// @pxl/client/service/core/source.ts

import * as source from "@/abi/source.ts";
import { core } from "@/service/core/core.ts";

/** Resolves false when this part of the declaration is more than one command
 * can carry, or when the core refuses it. */
export async function source_declare(
  min_zoom: number,
  formats: number[],
  shards: string[],
): Promise<boolean> {
  try {
    await source.declare(core.source, min_zoom, formats, shards);
    return true;
  } catch {
    return false;
  }
}
