// @pxl/api/modules/world/poi/providers/osm_provider.ts

import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import { type IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { FeatRow } from "@pxl/shared/bake/format.ts";
import type { OSMFeature, OSMRawFeature, POI } from "@pxl/shared/bake/model/osm_feature.ts";
import * as osm_adapter from "@pxl/api/bake/osm/model/osm_adapter.ts";
import { normalize, V3_ARENA } from "@pxl/api/bake/osm/pipeline/osm_normalizer.ts";
import { extract_pois } from "@pxl/api/bake/osm/pipeline/poi_extractor.ts";
import type { TerrainRepository } from "../../terrain/repository.ts";
import type { PoiProvider } from "../provider.ts";

/**
 * The only PoiProvider today (see provider.ts's own doc comment for why
 * this is behind an interface at all) — deliberately NOT terrain/source.ts's
 * fetch_terrain_source + bake_terrain. That pipeline exists to keep the
 * offline pre-baked tile and the live terrain endpoint byte-identical, which
 * matters for terrain because it's genuinely re-baked once and served as a
 * static asset from then on. This reuses the same DB query and normalization
 * steps for a different reason: OSM feature rows are the *only* thing POI
 * has ever come from, not because its own output needs to match some other
 * pipeline byte for byte.
 *
 * What this reuses from the terrain pipeline: `TerrainRepository.fetch_feat_rows`
 * (the one DB query this actually needs — POIs only ever come from OSM
 * feature rows, never elevation or coastline data) and the exact same
 * row -> OSMRawFeature -> normalize(OSMFeature) steps bake.ts's own
 * `bake_terrain` uses, so a POI's own extracted position agrees with
 * wherever the terrain this page bakes actually puts it. `extract_pois`
 * itself (poi_extractor.ts) is unchanged, pure, and already shared.
 *
 * V3_ARENA safety: bake_terrain (bake.ts) is a fully synchronous function —
 * its own `using V3_ARENA.scope` opens and disposes within one microtask,
 * so no other request's code can ever run in between and reuse the slots
 * it's still holding. `fetch_pois` here is async and awaits a DB round trip,
 * so the scope below is opened only AFTER that await settles — never
 * spanning a suspension point — and `normalize`'s own arena-backed `center`
 * array is cloned into a plain one before returning, since extract_pois()
 * hands back a direct reference into the arena, not a copy, and that slot
 * is recycled for reuse (by a concurrent fetch_pois/bake_terrain call, or a
 * later one) the instant this function's scope disposes at return. An
 * earlier version of this function opened the scope before the await and
 * returned extract_pois()'s output uncloned — confirmed live: the same
 * page_idx returned different `center` values on successive requests,
 * corrupted by whichever other request's normalize() call happened to
 * reuse the same recycled arena slot in between.
 */
export function make_osm_provider(repo: TerrainRepository): PoiProvider {
  return {
    async fetch_pois(page_idx: IVec): Promise<POI[]> {
      const page_bbox: BBox3857 = epsg3857.get_page_bbox(page_idx);
      const page_z_scale = epsg3857.Z_SCALE(page_bbox[0][1]);

      const feat_rows: FeatRow[] = await repo.fetch_feat_rows(page_bbox);

      using _ = V3_ARENA.scope;

      const feats: OSMFeature[] = [];
      for (const row of feat_rows) {
        const res: OSMRawFeature[] = osm_adapter.from_feat_row(row);
        for (const raw of res) {
          feats.push(normalize(raw, page_bbox[0], page_z_scale));
        }
      }

      const pois = extract_pois(feats);
      return pois.map((poi) => ({ ...poi, center: [...poi.center] as typeof poi.center }));
    },
  };
}
