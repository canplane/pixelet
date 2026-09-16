// @pxl/api/bake/terrain/pipeline/coast_policy.ts

/**
 * Coastline-only normalize policy — repository.ts's OVERTURE_COAST_SQL,
 * computed from overture_base_water's `ocean` subtype, feeds coast_adapter.
 * ts's from_coast_row, which synthesizes `{natural: "ocean", source:
 * "overture_ocean"}` tags for this file and normalizer.ts to key off of.
 */

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { type CellConfig, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import type { CoastRawFeature, Feature, FeatureExtra } from "@pxl/shared/bake/model/feature.ts";

import type { NormalizePolicy } from "./normalizer.ts";

const IS_COAST = (raw: CoastRawFeature): boolean => raw.tags.natural === "ocean" || raw.tags.source === "overture_ocean";

export const coast_policy: NormalizePolicy<CoastRawFeature> = {
  get_id: (raw) => raw.id,

  determine_category: (raw) => IS_COAST(raw) ? "sea" : "landuse",

  compute_metrics: (_raw, category): Feature["computed"] => ({
    height: 0,
    width: 0,
    layer: category === "sea" ? -5 : 0,
  }),

  should_force_closed: IS_COAST,

  // coastline polygons are the only thing left flowing through this policy — never linear.
  is_linear: () => false,

  compute_extra: (raw, category): FeatureExtra => {
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
      return { cell_type: cube_types.land.sea, ch_key: GroundChannelKey.LAND };
    }
    return null;
  },
};
