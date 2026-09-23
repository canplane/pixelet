// @pxl/web/cloudflare-worker/index.ts

/**
 * Cloudflare Worker that serves the app, its baked pages and assets, and
 * forward search's own live API — all from one origin.
 *
 * Static assets are handled by the platform before this script runs (see
 * wrangler.jsonc). What reaches this script is the roots of
 * spec/pxl/spec/route.json: `/geo/tiles/*` and `/asset/*` come straight out
 * of the R2 bucket, and the rest of `/geo/*` and all of `/api/*` are
 * reverse-proxied to GEO_ORIGIN and API_ORIGIN as-is (see infra/README.md).
 * Keeping it on the app's own hostname is what makes them same-origin, so no
 * CORS policy exists to configure or get wrong — see
 * issue/20260816-deployment-hosting.md.
 *
 * Lives outside src/ deliberately: this is the only server-side code in the
 * deployment, it targets the Workers runtime rather than the browser, and
 * `deno task check` covers src/ only. The handful of platform types it needs
 * are declared inline rather than pulling in @cloudflare/workers-types for
 * one file.
 */

type R2Object = {
  body: ReadableStream;
  httpEtag: string;
  writeHttpMetadata(headers: Headers): void;
};
type R2Bucket = {
  get(key: string): Promise<R2Object | null>;
  head(key: string): Promise<Omit<R2Object, "body"> | null>;
};

/** Cloudflare's ResponseInit extension: "manual" suppresses the runtime's
 * own content-encoding handling for a body that is already encoded. */
declare global {
  interface ResponseInit {
    encodeBody?: "auto" | "manual";
  }
  /** Workers' edge cache; `default` is the shared per-colo cache. */
  const caches: { default: { match(r: Request): Promise<Response | undefined>; put(r: Request, res: Response): Promise<void> } };
}

/** Third handler argument: `waitUntil` keeps work alive past the response. */
type ExecutionContext = { waitUntil(p: Promise<unknown>): void };

export type Env = {
  /** Bucket holding the pre-baked tiles, one object per page_idx. */
  TILES: R2Bucket;
  /** Static asset binding, used only for the fallthrough below. */
  ASSETS: { fetch(request: Request): Promise<Response> };
  /** The live API's own origin (infra/README.md) — an env var
   * (wrangler.jsonc's own `vars`), not a literal here, so pointing this at
   * a different box never needs a code change. */
  API_ORIGIN: string;
};

import { API_PREFIX, ASSET_PREFIX, GEO_PREFIX, TILE_PREFIX } from "./route.spec.ts";

/**
 * Object keys this Worker is willing to ask R2 for. Four shapes:
 *
 * - `terrain/<version>/<level>/<page_idx>`, plus the shared
 *   `terrain/<version>/_ocean` tile, which every level's open water reads. A
 *   page's key is within its level, so the level is its own segment.
 * - `poi/<version>/<page_idx>` — crates/pxl/server/geo-bake/src/poi/tool/bake_poi_tiles.rs's
 *   own sibling prefix in the same bucket. No `_ocean`-equivalent: a missing
 *   POI object means "no POI on this page", which poi/repository.ts resolves
 *   to `[]` directly, not a shared fallback object this Worker would need to
 *   know about.
 * - `geocode/<version>/<group_idx>`, plus the one `geocode/<version>/_dict`
 *   the whole set is read against. A reverse-geocode object covers a group
 *   of 16 pages a side rather than one page, and names the places its pages
 *   are in by number into that dictionary (pxl-spec bake/place_page.rs) — so
 *   this prefix has a shared object the way terrain does, and unlike POI. A
 *   missing group means "no page under it is in any place", which the
 *   kernel's layer/geocode/ reads as no label.
 * - `asset/<sha256>` — a model or an image, named by its content;
 *   pxl-asset-bake's ingest_asset writes them and the
 *   catalogs name them by path. No version segment: a changed file is a
 *   new key, which is what makes `immutable` true of it.
 *
 * page_idx is an L3 index printed as a decimal BigInt (see tile_repository.ts/
 * poi/repository.ts/geocode/repository.ts), so it can be negative; the
 * version segment is whatever the bake wrote as its output directory's last
 * path component (`v1` by default), kept permissive enough to survive a
 * rename. `poi`/`geocode` themselves are literals, not a permissive segment —
 * unlike the version, an unknown type prefix isn't a rename to allow for,
 * it's a new bake target this Worker needs its own line for (this stayed a
 * plain alternation rather than a generic segment for the two types there
 * are actually bake tools for; add to it when a third one ships, not before).
 *
 * This exists for cost, not correctness. A miss is still a billed R2 class-B
 * operation, so without it anyone can walk `/tile/<anything>` and charge
 * this account one read per request while every single one 404s. Matching
 * the shape first means a scraper's garbage never reaches the bucket at all.
 * It is not rate limiting and does not pretend to be — that needs WAF rules,
 * which need a real zone rather than a workers.dev hostname.
 */
const KEY_RE =
  /^(?:poi\/[A-Za-z0-9][A-Za-z0-9._-]{0,31}\/-?\d{1,20}|geocode\/[A-Za-z0-9][A-Za-z0-9._-]{0,31}\/(?:_dict|-?\d{1,20})|terrain\/[A-Za-z0-9][A-Za-z0-9._-]{0,31}\/(?:_ocean|\d{1,2}\/-?\d{1,20})|asset\/[0-9a-f]{64})$/;

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);

    // What the bucket answers, claimed first: the baked pages under the geo
    // root's tile branch, and the assets under their own root. An asset's key
    // *is* its path without the leading slash — the root and the bucket's
    // prefix are the same word — where a page's key is the tile branch removed.
    const bucket = url.pathname.startsWith(TILE_PREFIX) ? url.pathname.slice(TILE_PREFIX.length) : url.pathname.startsWith(ASSET_PREFIX) ? url.pathname.slice(1) : null;

    // Everything else under a claimed root is reverse-proxied to that root's
    // origin, same path, host header rewritten for Caddy. Neither branch reads
    // past its prefix.
    if (bucket === null) {
      const origin = url.pathname.startsWith(GEO_PREFIX) ? env.GEO_ORIGIN : url.pathname.startsWith(API_PREFIX) ? env.API_ORIGIN : null;
      if (origin) {
        const target = new URL(url.pathname + url.search, origin);
        const headers = new Headers(request.headers);
        headers.set("host", target.host);
        return fetch(new Request(target, { method: request.method, headers, body: request.body }));
      }
      return env.ASSETS.fetch(request);
    }

    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("method not allowed", { status: 405, headers: { allow: "GET, HEAD" } });
    }

    const key = bucket;
    // Rejected before R2 is touched — see KEY_RE. Same 404 a real miss
    // returns, deliberately: a distinct status would tell a prober which of
    // its guesses were the right shape, and the client cannot act on the
    // difference anyway (tile_repository.ts turns any 404 into the ocean
    // tile).
    if (!KEY_RE.test(key)) return new Response("not found", { status: 404 });

    // A missing object is not an error here: open-ocean pages are never
    // baked, and the client turns this 404 into its shared `_ocean` tile.
    // See tile_repository.ts.
    if (request.method === "HEAD") {
      const meta = await env.TILES.head(key);
      if (meta === null) return new Response(null, { status: 404 });
      const headers = new Headers();
      meta.writeHttpMetadata(headers);
      headers.set("etag", meta.httpEtag);
      return new Response(null, { headers });
    }

    // Edge cache in front of R2. Sound without any invalidation story because
    // terrain and assets are immutable and versioned by URL — a re-bake
    // publishes a new prefix rather than replacing bytes — and the POI and
    // geocode prefixes, which are rewritten in place monthly, carry a one-day
    // max-age the cache honours. This is also the only real cost control: R2 has no
    // spend limit, and class-B reads are the one meter that scales with
    // traffic, so every edge hit is a read that never happens.
    const cache = caches.default;
    const cached = await cache.match(request);
    if (cached) {
      // `encodeBody: "manual"` does not survive the cache round trip: the
      // entry comes back as an ordinary Response, and the runtime re-encodes
      // an already-encoded body on the way out. Re-assert it here.
      return new Response(cached.body, { headers: cached.headers, encodeBody: "manual" });
    }

    const object = await env.TILES.get(key);
    if (object === null) return new Response("not found", { status: 404 });

    // writeHttpMetadata replays what was set at upload time — crucially the
    // `content-encoding` the object was packed in, since objects are stored
    // pre-compressed and the client relies on fetch() transparently decoding
    // them.
    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);

    // `encodeBody: "manual"` is load-bearing, and its absence fails quietly.
    // By default the runtime assumes a Response body is *not* encoded and
    // applies its own compression, so an already-packed R2 object goes out
    // packed twice — measured on gzip: a stored 8,653-byte tile served as
    // 4,343 bytes to a gzip-accepting client, and as raw gzip bytes with no
    // content-encoding header at all to one asking for identity. Either way
    // decode_terrain() receives compressed bytes and fails far from here.
    // "manual" tells the runtime the body is already in its final encoding.
    //
    // `no-transform` is added on the way into the cache: without it the edge
    // is free to re-encode a stored response, which would reintroduce exactly
    // the double-compression this flag exists to prevent — just one layer
    // further away, and only on cache hits.
    const cc = headers.get("cache-control");
    if (cc && !cc.includes("no-transform")) headers.set("cache-control", `${cc}, no-transform`);

    const response = new Response(object.body, { headers, encodeBody: "manual" });
    ctx.waitUntil(cache.put(request, response.clone()));
    return response;
  },
};
