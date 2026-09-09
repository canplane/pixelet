// @pxl/api/bake/terrain/pipeline/coast_adapter.ts
//
// Coastline ingress adapter for repository.ts's OVERTURE_COAST_SQL rows —
// computed from overture_base_water's `ocean` subtype (see that query's own
// comment for why/how), not any OSM table.

import type { CoastRow } from "@pxl/shared/bake/format.ts";
import type { CoastRawFeature } from "@pxl/shared/bake/model/feature.ts";
import { wkb_to_geometries } from "@pxl/api/bake/wkb.ts";

export function from_coast_row(row: CoastRow): CoastRawFeature[] {
  if (row.covers_page) return []; // clipped full-page coast polygon => bogus sea fill candidate

  const geoms = wkb_to_geometries(row.geom_wkb);
  const out: CoastRawFeature[] = new Array(geoms.length);
  for (let i = 0; i < geoms.length; i++) {
    const g = geoms[i];
    out[i] = {
      id: row.id,
      tags: {
        natural: "ocean",
        source: "overture_ocean",
      },
      geometry: g,
      center: g.points[0] ?? [0, 0, 0],
    };
  }
  return out;
}
