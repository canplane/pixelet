// @pxl/api/module/tile/poi/service.ts

import type { Hono } from "@hono/hono";

import type { AppResources } from "@/app/resource.ts";
import * as env from "@/lib/env/mod.ts";

import type { u32 } from "@xpute/core/kit/word.ts";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { IVEC_WORD_MASK } from "@pxl/shared/spec/space/pvf3/core.ts";
import { L3_SHAMT } from "@pxl/shared/spec/space/indexer.ts";

import type { U8Array } from "@xpute/core/kit/array.ts";
import { encode_poi_page } from "@pxl/shared/bake/format.ts";
import { make_overture_provider } from "@pxl/api/bake/poi/provider/overture_provider.ts";
import type { PoiProvider } from "@pxl/api/bake/poi/provider.ts";

const MAX_L3_IDX = IVEC_WORD_MASK >> BigInt(L3_SHAMT);

/** One page's packet, and its gzip once a request has asked for one. */
type PageEntry = { raw: U8Array; gz: U8Array | null };

// Same page_idx validation terrain/service.ts uses — see that file's own
// doc comment on PAGE_IDX_PATTERN for why this is stricter than "parses as
// BigInt", not just a length guard.
const PAGE_IDX_PATTERN = /^\d{1,20}$/;

function make_services(provider: PoiProvider) {
  // POI, like terrain, is a pure function of static source data (Overture
  // Places here, Overture terrain features there — different tables, same
  // determinism) —
  // same cacheability reasoning as terrain/service.ts's own tile_cache, a
  // separate cache (own cap/env var) since POI pages are small packets, not
  // ~256KB terrain tiles, and shouldn't compete with that cache's capacity.
  // What is cached is the encoded page (format.ts's encode_poi_page): the
  // provider normalises, and from here on the page is bytes.
  const cache_cap = env.must_int("POI_CACHE_CAP", { fallback: 2000, min: 1 }) as u32;
  const poi_cache = new Map<IVec, PageEntry>();
  const in_flight = new Map<IVec, Promise<PageEntry>>();

  const get_poi_cached = async (page_idx: IVec): Promise<PageEntry> => {
    const cached = poi_cache.get(page_idx);
    if (cached) return cached;

    let pending = in_flight.get(page_idx);
    if (!pending) {
      pending = provider.fetch_pois(page_idx).then((pois) => ({ raw: encode_poi_page(pois), gz: null }));
      in_flight.set(page_idx, pending);
      pending.finally(() => in_flight.delete(page_idx));
    }

    const entry = await pending;
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
    poi_cache.set(page_idx, entry);
    return entry;
  };

  const invalidate = (page_idx?: IVec): void => {
    if (page_idx === undefined) poi_cache.clear();
    else poi_cache.delete(page_idx);
  };

  return { get_poi_cached, invalidate };
}

export function register(router: Hono, resrc: AppResources): void {
  const service = make_services(make_overture_provider(resrc.geo_pool));

  router.get("/api/tiles/poi/:page_idx", async (c) => {
    const raw_page_idx: string = c.req.param("page_idx");

    if (!PAGE_IDX_PATTERN.test(raw_page_idx)) {
      return c.text("bad page_idx", 400);
    }

    const page_idx: IVec = BigInt(raw_page_idx);
    if (page_idx > MAX_L3_IDX) {
      return c.text("bad page_idx", 400);
    }

    const entry = await service.get_poi_cached(page_idx);
    // The same arrangement as terrain/service.ts: hono's compress() skips
    // application/octet-stream, so the packet is gzipped here once per
    // page and stashed on the cached entry.
    if ((c.req.header("accept-encoding") ?? "").includes("gzip")) {
      if (!entry.gz) {
        const gzip_body = new Response(entry.raw).body!.pipeThrough(new CompressionStream("gzip"));
        entry.gz = new Uint8Array(await new Response(gzip_body).arrayBuffer());
      }
      return c.body(entry.gz, 200, { "content-type": "application/octet-stream", "content-encoding": "gzip" });
    }
    return c.body(entry.raw, 200, { "content-type": "application/octet-stream" });
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
