// @pxl/api/bake/terrain/pipeline/osm_policy.ts

/**
 * Trimmed to the sea/coastline branch only — `osm_elements`/`osm_roads`
 * (building/water/road/landuse/forest, everything this file used to also
 * handle) are replaced by overture_policy.ts. Coastline itself is now
 * Overture-sourced too (repository.ts's coast query, computed from
 * overture_base_water's `ocean` subtype rather than osm_coastline_polygons
 * — see that query's own comment), but from_coast_row's synthesized
 * `{natural: "ocean", source: "overture_ocean"}` tags keep flowing through
 * this file and normalizer.ts exactly as before the switch — the sea/land
 * bake logic never depended on which provider the polygon came from.
 */

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { type CellConfig, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import type { OSMExtra, OSMFeature, OSMRawFeature } from "@pxl/shared/bake/model/osm_feature.ts";

import type { NormalizePolicy } from "./normalizer.ts";

const IS_COAST = (raw: OSMRawFeature): boolean => raw.tags.natural === "coastline" || raw.tags.natural === "ocean" || raw.tags.natural === "sea" || raw.tags.source === "overture_ocean";

export const osm_policy: NormalizePolicy<OSMRawFeature> = {
  get_id: (raw) => raw.id,

  determine_category: (raw) => IS_COAST(raw) ? "sea" : "landuse",

  compute_metrics: (_raw, category): OSMFeature["computed"] => ({
    height: 0,
    width: 0,
    layer: category === "sea" ? -5 : 0,
  }),

  should_force_closed: IS_COAST,

  // coastline polygons are the only thing left flowing through this policy — never linear.
  is_linear: () => false,

  compute_extra: (raw, category): OSMExtra => {
    const is_ocean = IS_COAST(raw) || category === "sea";
    return {
      rank: is_ocean ? 110 : 10,
      is_ocean,
      is_parking: false,
      is_institutional: false,
      is_sports: false,
      is_tunnel: false, // from_coast_row never synthesizes a tunnel tag
    };
  },

  determine_cell_cfg: (_raw, category, extra): CellConfig | null => {
    if (extra.is_ocean || category === "sea") {
      return { cell_type: cube_types.land.SEA, ch_key: GroundChannelKey.LAND };
    }
    return null;
  },
};
