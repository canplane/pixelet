// @pxl/api/bake/osm/pipeline/poi_extractor.ts

import type { OSMFeature, POI } from "@pxl/shared/bake/model/osm_feature.ts";

/** Selects the notable places (POIs) worth showing on the map. */
export function extract_pois(feats: OSMFeature[]): POI[] {
  const pois: POI[] = [];

  for (const feat of feats) {
    const { tags, type, category, center, extra, id } = feat;
    const { name } = tags;

    // [Guard] the sea can never be a POI, plus required-data validation
    if (extra.is_ocean || !center || !name) continue;

    // [Refine] collection policy
    const is_explicit_poi = category === "poi";
    const is_major_facility = extra.is_institutional || extra.is_sports;

    // [Named Node] instead of taking every node, only keep ones with at
    // least minimal place info (amenity, shop, tourism, etc.)
    const is_named_node = type === "node" && (!!tags.amenity || !!tags.shop || !!tags.tourism || !!tags.place || !!tags.historic);

    // [Policy Check]
    if (is_explicit_poi || is_major_facility || is_named_node) {
      pois.push({
        id: `poi-${id}`,
        type: "poi",
        // priority: amenity > shop > tourism > category
        category: tags.amenity || tags.shop || tags.tourism || category || "place",
        name,
        center,
        rank: extra.rank, // already assigned by the normalizer
        tags,
      });
    }
  }

  // [Density Control] sort by rank (rendering-priority hint for the client)
  return pois.sort((a, b) => b.rank - a.rank);
}
