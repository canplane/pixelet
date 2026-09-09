// @pxl/api/modules/world/poi/service.ts

import type { Hono } from "@hono/hono";

import type { AppResources } from "@/app/resources.ts";
import * as env from "@/lib/env/mod.ts";

import type { u32 } from "@xpute/core/kit/word.ts";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { IVEC_WORD_MASK } from "@pxl/shared/spec/space/pvf3/core.ts";
import { L3_SHAMT } from "@pxl/shared/spec/space/indexer.ts";

import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";
import { make_overture_provider } from "@pxl/api/bake/poi/providers/overture_provider.ts";
import type { PoiProvider } from "@pxl/api/bake/poi/provider.ts";

const MAX_L3_IDX = IVEC_WORD_MASK >> BigInt(L3_SHAMT);

// Same page_idx validation terrain/service.ts uses — see that file's own
// doc comment on PAGE_IDX_PATTERN for why this is stricter than "parses as
// BigInt", not just a length guard.
const PAGE_IDX_PATTERN = /^\d{1,20}$/;

function make_services(provider: PoiProvider) {
  // POI, like terrain, is a pure function of static source data (Overture
  // Places here, OSM for terrain — different source, same determinism) —
  // same cacheability reasoning as terrain/service.ts's own tile_cache, a
  // separate cache (own cap/env var) since POI payloads are tiny JSON, not
  // ~256KB binary tiles, and shouldn't compete with that cache's capacity.
  const cache_cap = env.must_int("POI_CACHE_CAP", { fallback: 2000, min: 1 }) as u32;
  const poi_cache = new Map<IVec, POI[]>();
  const in_flight = new Map<IVec, Promise<POI[]>>();

  const get_poi_cached = async (page_idx: IVec): Promise<POI[]> => {
    const cached = poi_cache.get(page_idx);
    if (cached) return cached;

    let pending = in_flight.get(page_idx);
    if (!pending) {
      pending = provider.fetch_pois(page_idx);
      in_flight.set(page_idx, pending);
      pending.finally(() => in_flight.delete(page_idx));
    }

    const pois = await pending;
    // Insertion-order eviction, same shape as terrain/service.ts's LRU —
    // POI's own request pattern (a handful of visible pages, panning slowly
    // re-touches recent ones) doesn't need true recency reordering to stay
    // useful, so this stays a plain FIFO cap instead of duplicating that
    // file's own make_lru_cache helper for a payload shape different enough
    // (JSON array, not raw+gz bytes) that sharing it outright would need a
    // generic rewrite neither consumer asks for yet.
    if (poi_cache.size >= cache_cap) {
      const oldest = poi_cache.keys().next().value as IVec;
      poi_cache.delete(oldest);
    }
    poi_cache.set(page_idx, pois);
    return pois;
  };

  const invalidate = (page_idx?: IVec): void => {
    if (page_idx === undefined) poi_cache.clear();
    else poi_cache.delete(page_idx);
  };

  return { get_poi_cached, invalidate };
}

export function register(router: Hono, resrc: AppResources): void {
  const service = make_services(make_overture_provider(resrc.geo_pool));

  router.get("/api/world/poi/:page_idx", async (c) => {
    const raw_page_idx: string = c.req.param("page_idx");

    if (!PAGE_IDX_PATTERN.test(raw_page_idx)) {
      return c.text("bad page_idx", 400);
    }

    const page_idx: IVec = BigInt(raw_page_idx);
    if (page_idx > MAX_L3_IDX) {
      return c.text("bad page_idx", 400);
    }

    const pois = await service.get_poi_cached(page_idx);
    // POI's own V3 center tuples and plain-object tags serialize as JSON
    // directly, no bespoke binary encoding needed the way terrain's dense
    // per-cell channel bytes (format.ts's encode_terrain) benefit from —
    // main.ts's compress() middleware still gzips this transparently.
    return c.json(pois);
  });

  // gated by admin_auth on the /api/internal/* prefix (see main.ts)
  router.post("/api/internal/world/poi/cache/invalidate", async (c) => {
    const body = await c.req.json().catch(() => ({}) as { page_idx?: string });
    const raw_page_idx = body.page_idx;

    if (raw_page_idx === undefined) {
      service.invalidate();
      return c.text("invalidated all", 200);
    }

    if (!PAGE_IDX_PATTERN.test(raw_page_idx)) {
      return c.text("bad page_idx", 400);
    }
    const page_idx: IVec = BigInt(raw_page_idx);
    if (page_idx > MAX_L3_IDX) {
      return c.text("bad page_idx", 400);
    }

    service.invalidate(page_idx);
    return c.text(`invalidated ${raw_page_idx}`, 200);
  });
}
