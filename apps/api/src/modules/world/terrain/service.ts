// @pxl/api/modules/world/terrain/service.ts

import type { Hono } from "@hono/hono";

import type { AppResources } from "@/app/resources.ts";
import * as env from "@/lib/env/mod.ts";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Bytes } from "@xpute/core/kit/array.ts";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { IVEC_WORD_MASK } from "@pxl/shared/spec/space/pvf3/core.ts";

import { L3_SHAMT } from "@pxl/shared/spec/space/indexer.ts";
import { TerrainRepository } from "@pxl/api/bake/terrain/repository.ts";
import { fetch_tile } from "@pxl/api/bake/terrain/source.ts";

const MAX_L3_IDX = IVEC_WORD_MASK >> BigInt(L3_SHAMT);

/**
 * Digits only, capped well above MAX_L3_IDX's own ~18-digit width (2^57 ≈
 * 1.44e17) but far short of a string long enough to make BigInt()'s own
 * O(n^2)-ish decimal parse expensive — checked *before* BigInt() ever
 * touches attacker-controlled input, so an oversized digit string is
 * rejected as cheaply as a malformed one instead of paying to parse it
 * first. BigInt()'s constructor also accepts "-", "+", "0x…"/"0b…"/"0o…",
 * and surrounding whitespace, any of which could still parse to a value
 * outside page_idx's real domain below — this regex is stricter than
 * "parses as BigInt", not just a length guard.
 */
const PAGE_IDX_PATTERN = /^\d{1,20}$/;

/** Baked tile bytes, plus the gzip encoding of them once a caller has needed it. */
type TileCacheEntry = { raw: Bytes; gz: Bytes | null };

/** Insertion-order Map used as a fixed-capacity LRU: a `get` re-inserts its
 * key to bump recency, so the oldest (least-recently-used) key is always
 * the first one `.keys()` yields once the cap is exceeded. */
function make_lru_cache<K, V>(cap: u32) {
  const map = new Map<K, V>();
  return {
    get(key: K): V | undefined {
      const v = map.get(key);
      if (v === undefined) return undefined;
      map.delete(key);
      map.set(key, v);
      return v;
    },
    set(key: K, value: V): void {
      map.delete(key);
      map.set(key, value);
      if (map.size > cap) {
        const oldest = map.keys().next().value as K;
        map.delete(oldest);
      }
    },
    delete(key: K): void {
      map.delete(key);
    },
    clear(): void {
      map.clear();
    },
  };
}

function make_services(repo: TerrainRepository) {
  // page_idx -> baked bytes is a pure function of static source data (OSM +
  // DEM don't change underneath a running server), so it's cacheable as-is.
  const cache_cap = env.must_int("TERRAIN_TILE_CACHE_CAP", { fallback: 500, min: 1 }) as u32;
  const tile_cache = make_lru_cache<IVec, TileCacheEntry>(cache_cap);
  // Coalesces concurrent requests for a still-uncached page_idx into a
  // single bake instead of one DB round trip per request — without this,
  // N simultaneous requests for the same cold page_idx (e.g. everyone
  // panning into a newly-visible tile at once) would independently repeat
  // the full fetch_terrain_source + bake_terrain pipeline.
  const in_flight = new Map<IVec, Promise<Bytes>>();

  const get_tile_cached = async (page_idx: IVec): Promise<TileCacheEntry> => {
    const cached = tile_cache.get(page_idx);
    if (cached) return cached;

    let pending = in_flight.get(page_idx);
    if (!pending) {
      pending = fetch_tile(repo, page_idx);
      in_flight.set(page_idx, pending);
      pending.finally(() => in_flight.delete(page_idx));
    }

    const raw = await pending;
    const entry: TileCacheEntry = { raw, gz: null };
    tile_cache.set(page_idx, entry);
    return entry;
  };

  // page_idx omitted -> drop everything (e.g. after a re-bake pass or a
  // fix like the ocean/land ground_baker.ts change). Only affects the
  // cache; a request already in flight for `page_idx` still lands and
  // repopulates it with whatever it fetched, invalidation or not.
  const invalidate = (page_idx?: IVec): void => {
    if (page_idx === undefined) tile_cache.clear();
    else tile_cache.delete(page_idx);
  };

  return { get_tile_cached, invalidate };
}

export function register(router: Hono, resrc: AppResources): void {
  const repo = new TerrainRepository(resrc.geo_pool);
  const service = make_services(repo);

  router.get("/api/world/terrain/tiles/:page_idx", async (c) => {
    const raw_page_idx: string = c.req.param("page_idx");

    // page_idx is an L3_IDX (see indexer.ts): an *unsigned* logical key in
    // [0, MAX_L3_IDX]. A negative or out-of-range value isn't just
    // "invalid" the way a NaN would be — FROM_L3_IDX/get_page_bbox happily
    // shift and unproject it into some nonsense-but-well-formed bbox with
    // no exception anywhere along the way, which would've reached the DB
    // as a garbage query instead of failing fast here.
    if (!PAGE_IDX_PATTERN.test(raw_page_idx)) {
      return c.text("bad page_idx", 400);
    }

    const page_idx: IVec = BigInt(raw_page_idx);
    if (page_idx > MAX_L3_IDX) {
      return c.text("bad page_idx", 400);
    }

    const entry = await service.get_tile_cached(page_idx);

    // hono's own compress() middleware (main.ts) skips this response: it
    // gates on Content-Type, and application/octet-stream isn't in its
    // compressible-type allowlist (reasonably so in general — opaque binary
    // is often already-compressed media). This payload isn't that: it's
    // dense per-cell ground-channel bytes built from polygon fills, mostly
    // large uniform runs (see format.ts's encode_terrain) — gzips well.
    // fetch() on the client decodes Content-Encoding transparently, so this
    // needs no client-side change.
    if ((c.req.header("accept-encoding") ?? "").includes("gzip")) {
      // entry is the same cached object every hit reaches, so compressing
      // once and stashing it back means every subsequent request for this
      // page_idx (gzip-accepting or not) skips CompressionStream entirely.
      if (!entry.gz) {
        const gzip_body = new Response(entry.raw).body!.pipeThrough(new CompressionStream("gzip"));
        entry.gz = new Uint8Array(await new Response(gzip_body).arrayBuffer());
      }
      return c.body(entry.gz, 200, { "content-type": "application/octet-stream", "content-encoding": "gzip" });
    }

    return c.body(entry.raw, 200, { "content-type": "application/octet-stream" });
  });

  // gated by admin_auth on the /api/internal/* prefix (see main.ts)
  router.post("/api/internal/world/terrain/cache/invalidate", async (c) => {
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
