// @pxl/api/modules/search/service.ts
//
// Forward search's own live endpoint — the one world/ feature that can't
// be pre-baked like terrain/poi/geocode (see provider.ts's own top comment
// on why): a free-text query is an unbounded input space, not a finite L3
// page, so this always hits Postgres directly rather than serving a
// pre-gzipped static file. No cache layer the way poi/geocode/service.ts
// have one — repeat identical queries are cheap enough against a trigram-
// indexed column that a cache would mostly just add complexity for a
// feature whose whole point is that *different* queries are the norm.

import type { Hono } from "@hono/hono";

import type { AppResources } from "@/app/resources.ts";

import { make_overture_search_provider } from "./providers/overture_provider.ts";
import type { SearchProvider } from "./provider.ts";

const MIN_QUERY_LEN = 1;
const MAX_QUERY_LEN = 100;
const DEFAULT_LIMIT = 20;
const MAX_LIMIT = 50;

function parse_near(raw: string | undefined): { lon: number; lat: number } | undefined {
  if (!raw) return undefined;
  const parts = raw.split(",");
  if (parts.length !== 2) return undefined;
  const lon = Number(parts[0]);
  const lat = Number(parts[1]);
  if (!Number.isFinite(lon) || !Number.isFinite(lat)) return undefined;
  return { lon, lat };
}

export function register(router: Hono, resrc: AppResources): void {
  const provider: SearchProvider = make_overture_search_provider(resrc.geo_pool);

  // Under /api because that is the one prefix the relays forward — see
  // infra/prod/README.md. A bare path is unreachable in production.
  router.get("/api/search", async (c) => {
    const q = (c.req.query("q") ?? "").trim();
    if (q.length < MIN_QUERY_LEN || q.length > MAX_QUERY_LEN) {
      return c.json({ error: "bad query" }, 400);
    }

    const near = parse_near(c.req.query("near"));
    const raw_limit = c.req.query("limit");
    const limit = raw_limit ? Math.min(Math.max(Number(raw_limit) || DEFAULT_LIMIT, 1), MAX_LIMIT) : DEFAULT_LIMIT;

    const results = await provider.search(q, { near, limit });
    return c.json(results);
  });
}
