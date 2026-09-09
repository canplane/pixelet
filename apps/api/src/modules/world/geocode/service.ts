// @pxl/api/modules/world/geocode/service.ts

import type { Hono } from "@hono/hono";

import type { AppResources } from "@/app/resources.ts";
import * as env from "@/lib/env/mod.ts";

import type { u32 } from "@xpute/core/kit/word.ts";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { IVEC_WORD_MASK } from "@pxl/shared/spec/space/pvf3/core.ts";
import { L3_SHAMT } from "@pxl/shared/spec/space/indexer.ts";

import type { GeocodeProvider, GeocodeResult } from "./provider.ts";
import { make_overture_geocode_provider } from "./providers/overture_provider.ts";

const MAX_L3_IDX = IVEC_WORD_MASK >> BigInt(L3_SHAMT);

// Same page_idx validation poi/service.ts and terrain/service.ts share.
const PAGE_IDX_PATTERN = /^\d{1,20}$/;

function make_services(provider: GeocodeProvider) {
  // A page's neighborhood label is a pure function of static Divisions
  // data, same cacheability reasoning poi/service.ts's own cache documents
  // — a separate cache (own cap) since a geocode result is a handful of
  // bytes, not POI's small-JSON-array or terrain's ~256KB binary tile.
  const cache_cap = env.must_int("GEOCODE_CACHE_CAP", { fallback: 4000, min: 1 }) as u32;
  const cache = new Map<IVec, GeocodeResult | null>();
  const in_flight = new Map<IVec, Promise<GeocodeResult | null>>();

  const get_cached = async (page_idx: IVec): Promise<GeocodeResult | null> => {
    if (cache.has(page_idx)) return cache.get(page_idx)!;

    let pending = in_flight.get(page_idx);
    if (!pending) {
      pending = provider.reverse(page_idx);
      in_flight.set(page_idx, pending);
      pending.finally(() => in_flight.delete(page_idx));
    }

    const result = await pending;
    if (cache.size >= cache_cap) {
      const oldest = cache.keys().next().value as IVec;
      cache.delete(oldest);
    }
    cache.set(page_idx, result);
    return result;
  };

  const invalidate = (page_idx?: IVec): void => {
    if (page_idx === undefined) cache.clear();
    else cache.delete(page_idx);
  };

  return { get_cached, invalidate };
}

export function register(router: Hono, resrc: AppResources): void {
  const service = make_services(make_overture_geocode_provider(resrc.geo_pool));

  router.get("/api/world/geocode/:page_idx", async (c) => {
    const raw_page_idx: string = c.req.param("page_idx");

    if (!PAGE_IDX_PATTERN.test(raw_page_idx)) {
      return c.text("bad page_idx", 400);
    }

    const page_idx: IVec = BigInt(raw_page_idx);
    if (page_idx > MAX_L3_IDX) {
      return c.text("bad page_idx", 400);
    }

    const result = await service.get_cached(page_idx);
    if (result === null) return c.json(null, 404);
    return c.json(result);
  });

  // gated by admin_auth on the /api/internal/* prefix (see main.ts)
  router.post("/api/internal/world/geocode/cache/invalidate", async (c) => {
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
