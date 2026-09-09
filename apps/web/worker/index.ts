// @pxl/web/../worker/index.ts

/**
 * Cloudflare Worker that serves the app, its terrain tiles, and forward
 * search's own live API — all from one origin.
 *
 * Static assets are handled by the platform before this script runs (see
 * wrangler.jsonc); what is routed here is `/api/*`, split into `/api/tiles/*`
 * (read straight out of the R2 bucket) and everything else (reverse-proxied
 * to API_ORIGIN as-is, see infra/prod/README.md). Keeping it on the app's
 * own hostname is what
 * makes them same-origin, so no CORS policy exists to configure or get
 * wrong — see discussions/20260816-deployment-hosting.md.
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
  /** The live API's own origin (infra/prod/README.md) — an env var
   * (wrangler.jsonc's own `vars`), not a literal here, so pointing this at
   * a different box never needs a code change. */
  API_ORIGIN: string;
};

/**
 * Everything the server answers sits under one root, and the only split
 * inside it is where the bytes come from: a bucket for what was baked
 * ahead of time, the live API for everything else. Neither branch reads
 * further — whether the API stays a set of REST routes or becomes a single
 * RPC root is undecided, and a relay that enumerates route names is
 * rewritten by that decision. Adding a live route is an API change and
 * nothing else.
 *
 * Mirrors src/lib/env/routes.ts's EDGE_* prefixes. Kept as literals because
 * this file targets the Workers runtime and is bundled separately from src/
 * — wrangler.jsonc's `run_worker_first` carries the root for the same
 * reason.
 */
const API_PREFIX = "/api/";
const PREFIX = "/api/tiles/";

/**
 * Object keys this Worker is willing to ask R2 for. Three shapes:
 *
 * - `terrain/<version>/<page_idx>`, plus the shared `terrain/<version>/_ocean`
 *   tile. The bare `<version>/…` form without the type segment is also
 *   accepted, which is what this bucket held when it held only terrain.
 * - `poi/<version>/<page_idx>` — apps/api/src/bake/poi/tools/bake_poi_tiles.ts's
 *   own sibling prefix in the same bucket. No `_ocean`-equivalent: a missing
 *   POI object means "no POI on this page", which poi/repository.ts resolves
 *   to `[]` directly, not a shared fallback object this Worker would need to
 *   know about.
 * - `geocode/<version>/<page_idx>` — apps/api/src/bake/geocode/tools/
 *   bake_geocode_tiles.ts's own sibling prefix, same no-fallback shape as
 *   POI (a missing object means "outside Overture's Divisions coverage",
 *   geocode/repository.ts resolves that to `null` directly).
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
 * operation, so without it anyone can walk `/tiles/<anything>` and charge
 * this account one read per request while every single one 404s. Matching
 * the shape first means a scraper's garbage never reaches the bucket at all.
 * It is not rate limiting and does not pretend to be — that needs WAF rules,
 * which need a real zone rather than a workers.dev hostname.
 */
const KEY_RE =
  /^(?:(?:poi|geocode)\/[A-Za-z0-9][A-Za-z0-9._-]{0,31}\/-?\d{1,20}|terrain\/[A-Za-z0-9][A-Za-z0-9._-]{0,31}\/(?:_ocean|-?\d{1,20})|[A-Za-z0-9][A-Za-z0-9._-]{0,31}\/(?:_ocean|-?\d{1,20}))$/;

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);

    // Tiles are a prefix inside the API root, so they are claimed first;
    // everything else under that root is reverse-proxied to API_ORIGIN,
    // same path, host header rewritten for Caddy.
    if (!url.pathname.startsWith(PREFIX)) {
      if (url.pathname.startsWith(API_PREFIX)) {
        const target = new URL(url.pathname + url.search, env.API_ORIGIN);
        const headers = new Headers(request.headers);
        headers.set("host", target.host);
        return fetch(new Request(target, { method: request.method, headers, body: request.body }));
      }
      // `run_worker_first` should mean this only ever sees the API root, but
      // the guard keeps the Worker correct on its own rather than relying on
      // routing config staying in sync with it.
      return env.ASSETS.fetch(request);
    }

    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("method not allowed", { status: 405, headers: { allow: "GET, HEAD" } });
    }

    const key = url.pathname.slice(PREFIX.length);
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
    // tiles are immutable and versioned by URL — a re-bake publishes a new
    // prefix rather than replacing bytes, so a cached entry can never go
    // stale. This is also the only real cost control available: R2 has no
    // spend limit, and class-B reads are the one meter that scales with
    // traffic, so every edge hit is a read that never happens.
    const cache = caches.default;
    const cached = await cache.match(request);
    if (cached) {
      // `encodeBody: "manual"` does not survive the cache round trip: the
      // entry comes back as an ordinary Response, and the runtime re-encodes
      // an already-gzipped body on the way out. Re-assert it here.
      return new Response(cached.body, { headers: cached.headers, encodeBody: "manual" });
    }

    const object = await env.TILES.get(key);
    if (object === null) return new Response("not found", { status: 404 });

    // writeHttpMetadata replays what was set at upload time — crucially
    // `content-encoding: gzip`, since objects are stored pre-compressed and
    // the client relies on fetch() transparently decoding them.
    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);

    // `encodeBody: "manual"` is load-bearing, and its absence fails quietly.
    // By default the runtime assumes a Response body is *not* encoded and
    // applies its own compression, so an already-gzipped R2 object goes out
    // as gzip-inside-gzip — measured: a stored 8,653-byte tile served as
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
