// @pxl/client/service/core/source.ts

import { Command } from "@/abi/cmd.spec.ts";
import { core } from "@/service/core/core.ts";

/** Returns false when this part of the declaration is more than one command
 * can carry, or when the core refuses it. */
export function source_declare(
  min_zoom: number,
  formats: number[],
  shards: string[],
): boolean {
  return (
    core.call(
      Command.SOURCE_DECLARE,
      min_zoom,
      Uint32Array.from(formats),
      shards,
    ) >= 0
  );
}
