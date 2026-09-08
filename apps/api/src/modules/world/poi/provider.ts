// @pxl/api/modules/world/poi/provider.ts

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";

/**
 * Where POI data actually comes from — one page_idx in, that page's POIs
 * out. service.ts (the live endpoint) and bake/poi/tools/bake_poi_tiles.ts
 * (the offline static bake) both depend on this, not on any one
 * implementation — the actual reason it exists: OSM's own data staleness
 * (how often a re-bake is even worth doing) is a property of *this*
 * provider specifically, not of POI as a concept. A commercial provider
 * (Naver/Kakao Local Search, say) would have a completely different
 * freshness model — live queries, no re-bake at all — so that decision
 * belongs behind this seam, not hardcoded into either consumer.
 * providers/osm_provider.ts is the only implementation today.
 */
export type PoiProvider = {
  fetch_pois(page_idx: IVec): Promise<POI[]>;
};
