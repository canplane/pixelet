// @pxl/client/ui/division/pivot.ts

import type { LonLat } from "@/abi/cmd.spec.ts";
import { assembleLabel } from "./label.ts";
import { fixed } from "@/ui/fixed.ts";
import { useCore } from "@/ui/store/watch.ts";

/** The pivot's division, assembled per snapshot; the core says which of its
 * names the view is in far enough to show. */
export function useDivisionLabel(): string | null {
  return useCore((s) => {
    const parts = s.division.filter((p) => p.shown);
    return parts.length === 0 ? null : assembleLabel(parts);
  });
}

export function usePivotCoordinates(): string | null {
  return useCore((s) => s.pivot.lon_lat && coordinateLabel(s.pivot.lon_lat));
}

/** Four decimals: about eleven meters. */
function coordinateLabel({ lat, lon }: LonLat): string {
  return `${fixed(Math.abs(lat), 4)}° ${lat < 0 ? "S" : "N"}, ${fixed(
    Math.abs(lon),
    4,
  )}° ${lon < 0 ? "W" : "E"}`;
}
