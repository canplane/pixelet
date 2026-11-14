// @pxl/client/ui/feature/division/pivot.ts

import type { LonLat } from "@/abi/overlay.ts";
import { assemble_label } from "./label.ts";
import { fixed } from "@/ui/shared/fixed.ts";
import { useCore } from "@/ui/shared/watch.ts";

/** The core marks which of the division's names the view is zoomed in far
 * enough to show. */
export function useDivisionLabel(): string | null {
  return useCore((s) => {
    const parts = s.division.filter((p) => p.shown);
    return parts.length === 0 ? null : assemble_label(parts);
  });
}

export function usePivotCoordinate(): string | null {
  return useCore((s) => s.pivot.lon_lat && coordinate_label(s.pivot.lon_lat));
}

/** Four decimals of a degree are about eleven meters. */
function coordinate_label({ lat, lon }: LonLat): string {
  return `${fixed(Math.abs(lat), 4)}° ${lat < 0 ? "S" : "N"}, ${fixed(
    Math.abs(lon),
    4,
  )}° ${lon < 0 ? "W" : "E"}`;
}
