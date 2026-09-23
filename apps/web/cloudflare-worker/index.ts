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

/** Where a stored entry keeps the encoding its bytes are in. A header the
 * cache has no meaning for is a header it leaves alone, which is the whole
 * point: `content-encoding` is what makes it normalise an entry. */
const STORED_ENCODING = "x-pxl-stored-encoding";

/** Whether the client will take a body in this encoding: the first exact name
 * in `accept-encoding` decides, and `*` stands in for a name that is not
 * listed. A `q` of zero is a refusal, which is how a client spells "not that
 * one" without leaving the name out. */
function accepts(request: Request, encoding: string): boolean {
  let wildcard: number | null = null;
  for (const part of (request.headers.get("accept-encoding") ?? "").split(",")) {
    const [name, ...params] = part.trim().split(";");
    const exact = name.toLowerCase() === encoding;
    if (!exact && name !== "*") continue;
    const q = params.map((p) => p.trim().toLowerCase()).find((p) => p.startsWith("q="));
    const weight = q === undefined ? 1 : Number(q.slice(2));
    if (exact) return weight > 0;
    wildcard = Number.isNaN(weight) ? 0 : weight;
  }
  return wildcard !== null && wildcard > 0;
}

/** An entry on its way to the client: the encoding back where HTTP expects it,
 * and the body declared final so nothing encodes it a second time.
 *
 * `encodeBody: "manual"` is load-bearing here and its absence fails quietly.
 * By default the runtime assumes a Response body is *not* encoded and applies
 * its own compression, so an already-packed object goes out packed twice —
 * measured on gzip: a stored 8,653-byte tile served as 4,343 bytes to a
 * gzip-accepting client, and as raw gzip bytes with no content-encoding header
 * at all to one asking for identity. Either way decode_terrain() receives
 * compressed bytes and fails far from here.
 *
 * A client that did not offer the encoding is **refused, not accommodated**.
 * Left alone, the edge negotiates on this deployment's behalf and decodes the
 * body for such a client. Measured across the prefixes, zstd-accepting against
 * not, 2026-09-24: a terrain page 8,543 against 526,936, the shared ocean page
 * 101 against the same 526,936 — five thousand times the bytes for the object
 * the most requests are for — the geocode dictionary 90,141 against 619,224,
 * a geocode group 2,746 against 12,192. A terrain page is a fixed-size raster
 * and packs to almost nothing, so the emptier the page the worse the ratio.
 * That is on an account whose only spend control is that a request is small
 * and that Workers fails closed at the free plan's daily count
 * (issue/20260816-deployment-hosting.md), and the decoded copy is what the
 * edge then caches. Nothing here can teach a browser to read zstd, so the
 * answer is the one HTTP already has a status for: an object goes out in the
 * encoding it is stored in or not at all, and the client says so to whoever
 * is looking at it (kernel/io/fetch.ts). */
function serve(request: Request, body: ReadableStream | null, from: Headers): Response {
  const headers = new Headers(from);
  const encoding = headers.get(STORED_ENCODING);
  if (encoding) {
    if (!accepts(request, encoding)) return new Response(`stored as ${encoding}, which this client did not accept`, { status: 406 });
    headers.delete(STORED_ENCODING);
    headers.set("content-encoding", encoding);
  }
  return new Response(body, { headers, encodeBody: "manual" });
}

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
    //
    // What is cached is the packed bytes, and the client is what unpacks them.
    // That is not what caching the response as it stands does: an entry whose
    // `content-encoding` the cache can read is one the cache normalises, and
    // it stores the *decoded* body, marks the entry `vary: accept-encoding`
    // and then declines to compress it again, because `application/octet-stream`
    // is not one of the types Cloudflare compresses. Measured on one URL,
    // miss then hit, seconds apart: 8,543 zstd bytes out of R2, and 526,936
    // raw bytes back off the hit with no encoding header at all — a hit
    // carrying sixty-two times the bytes of the miss it was meant to save
    // (2026-09-23). Nothing failed, because the client is happy either way;
    // it only cost.
    //
    // So the encoding is kept where the cache cannot read it. `STORED_ENCODING`
    // holds it while the entry is stored, the body goes in as the opaque
    // octets it is, and both paths out put it back on the way to the client.
    const cache = caches.default;
    const cached = await cache.match(request);
    if (cached) return serve(request, cached.body, cached.headers);

    const object = await env.TILES.get(key);
    if (object === null) return new Response("not found", { status: 404 });

    // writeHttpMetadata replays what was set at upload time — crucially the
    // `content-encoding` the object was packed in, since objects are stored
    // pre-compressed and the client relies on fetch() transparently decoding
    // them.
    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);

    // `no-transform` so that nothing re-encodes what is already in its final
    // encoding, on the way out or out of the cache.
    const cc = headers.get("cache-control");
    if (cc && !cc.includes("no-transform")) headers.set("cache-control", `${cc}, no-transform`);

    const encoding = headers.get("content-encoding");
    if (encoding) {
      headers.delete("content-encoding");
      headers.set(STORED_ENCODING, encoding);
    }

    // The body is stored as it lies. `encodeBody: "manual"` is not needed on
    // this one — there is no `content-encoding` for the runtime to act on —
    // and it would not survive `clone()` into the cache anyway.
    const stored = new Response(object.body, { headers });
    ctx.waitUntil(cache.put(request, stored.clone()));
    return serve(request, stored.body, headers);
  },
};
