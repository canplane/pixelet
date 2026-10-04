// @pxl/web/cloudflare-worker/index.ts

/**
 * Cloudflare Worker that serves the app, its baked pages and assets, and
 * forward search's own live API — all from one origin.
 *
 * Static assets are handled by the platform before this script runs (see
 * wrangler.jsonc). What reaches this script is the roots of
 * spec/pxl/spec/route.json: `/tile/*` and `/asset/*` come straight out of the
 * R2 bucket, and `/geo/*` and `/api/*` are reverse-proxied to GEO_ORIGIN and API_ORIGIN as-is (see infra/README.md).
 * Keeping it on the app's own hostname is what makes them same-origin, so no
 * CORS policy exists to configure or get wrong — see
 * decision/20260816-deployment-hosting.md.
 *
 * Lives outside src/ deliberately: this is the only server-side code in the
 * deployment, it targets the Workers runtime rather than the browser, and
 * `deno task check` covers src/ only. The handful of platform types it needs
 * are declared inline rather than pulling in @cloudflare/workers-types for
 * one file.
 */

type R2Object = {
  body: ReadableStream;
  arrayBuffer(): Promise<ArrayBuffer>;
  text(): Promise<string>;
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
  interface CacheStorage {
    readonly default: Cache;
  }
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
  /** The geo server's origin, set the same way. */
  GEO_ORIGIN: string;
};

import { API_PREFIX, ASSET_PREFIX, GEO_PREFIX, TILE_PREFIX } from "./route.spec.ts";

/**
 * Object keys this Worker is willing to ask R2 for.
 *
 * A baked page is named by its **quadkey**: its place in its level's page
 * grid, one base-4 digit a halving, coarsest first (pxl-spec
 * bake/quadkey.rs). Nothing else is written — the number of digits says the
 * level, and the digits are the index — so the three bakes differ only
 * in how long a name they have and which shared object they keep beside it:
 *
 * - `terrain/<version>/<quadkey>`, 5 to 15 digits, a page at every level of
 *   the pyramid from the leaf up to `TOP_LEVEL`. Plus the shared
 *   `_ocean` page every level's open water reads.
 * - `poi/<version>/<quadkey>`, 5 to 15 digits, a page at every level of the
 *   pyramid as terrain's are: how many places stand in each of its cells. A
 *   page with none is no object.
 * - `geocode/<version>/<quadkey>`, 11 digits, because a reverse-geocode object
 *   covers a group of 16 pages a side (pxl-spec bake/place_page.rs's
 *   `GROUP_SHAMT`). Plus the one `_dict` the whole set is read against, since
 *   a group names the places its pages are in by number into it. A missing
 *   group means "no page under it is in any place", which the kernel's
 *   layer/geocode/ reads as no label.
 * - `asset/<path>/<stem>-<12 hex>.<ext>` — a model or an image under the tree
 *   it lives in, with a hash of its bytes in the stem, the way a bundler names
 *   a chunk. pxl-tool's ingest_asset writes them and the catalogs name
 *   them by path. No version segment and none wanted: a changed file is a new
 *   name, which is what makes `immutable` true of it.
 *
 * The version is `v<format>.<YYYY-MM-DD>.<n>`, a format and the day it was
 * baked, and this pattern does not read which one: the client asks for
 * whatever `published.json` says, so a new one is a client change and never a
 * deploy of this file. A bare `v<format>` is a client from before bakes were
 * named by day, still open in a tab.
 *
 * **This exists for cost, not correctness.** A miss is still a billed R2
 * class-B operation, so without it anyone can walk the tile root and charge
 * this account one read per request while every single one 404s. Matching the
 * shape first means a scraper's garbage never reaches the bucket at all. It is
 * not rate limiting and does not pretend to be — that needs WAF rules, which
 * need a real zone rather than a workers.dev hostname.
 *
 * What it has to sieve is now almost nothing. The old shape spelled a page as
 * a decimal index of up to twenty digits under a level of up to two, so the
 * key space was some 10^20 strings a level and all but a handful named
 * nothing. A quadkey of a given length is a page and there are no others, so
 * the key space here *is* the address space and this pattern turns away only
 * what was never an address. An asset is the one key nothing computes — the
 * shell looks it up in a table it ships — so its shape is all there is to
 * match, and the hash in the stem is what makes that shape worth matching.
 */
const VERSION = String.raw`v\d{1,3}(?:\.\d{4}-\d{2}-\d{2}\.\d+)?`;
const KEY_RE = new RegExp(
  String
    .raw`^(?:terrain\/${VERSION}\/(?:_ocean|[0-3]{5,15})|poi\/${VERSION}\/[0-3]{5,15}|geocode\/${VERSION}\/(?:_dict|[0-3]{11})|asset\/(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9._-]+-[0-9a-f]{12}\.(?:glb|png|webp))$`,
);

/** Where a stored entry keeps the encoding its bytes are in. A header the
 * cache has no meaning for is a header it leaves alone, which is the whole
 * point: `content-encoding` is what makes it normalize an entry. */
const STORED_ENCODING = "x-pxl-stored-encoding";

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
 * **A client that cannot read the encoding cannot be refused here, and this is
 * where that was learned.** The edge decodes the body for a client that did not
 * offer zstd, and the decoded copy is what the cache then holds: measured
 * 2026-09-24, a terrain page 8,543 bytes against 526,936, the shared ocean page
 * 101 against the same 526,936. Refusing such a client with 406 looks like the
 * answer and cannot be written here — Cloudflare rewrites `accept-encoding` on
 * the request a Worker is handed and negotiates with the real client itself on
 * the way out, so what this code reads is the edge's own list and not the
 * browser's. Every variant was refused when it was tried, `*` and `zstd`
 * included. Whatever refuses a client for its encoding has to run where the
 * client's own header is still intact, which is not here. */
function serve(body: ArrayBuffer | ReadableStream | null, from: Headers): Response {
  const headers = new Headers(from);
  const encoding = headers.get(STORED_ENCODING);
  if (encoding) {
    headers.delete(STORED_ENCODING);
    headers.set("content-encoding", encoding);
  }
  return new Response(body, { headers, encodeBody: "manual" });
}

/**
 * A partial version is its changes over one full version: `_snapshot` beside
 * its pages names that version and the pages it writes or removes, and every
 * other page is the full version's. The list in the bucket decides; one read
 * is held, since a version never changes what it lists, but its absence is
 * not, so a list uploaded after a version was first asked for still counts.
 * A version with no list is full.
 */
interface Snapshot {
  base: string;
  written: Set<string>;
  removed: Set<string>;
}

const SNAPSHOT_KEY = "_snapshot";
const BAKE_KEY = /^(terrain|poi|geocode)\/([^/]+)\/(.+)$/;
const snapshots = new Map<string, Snapshot>();

async function snapshotOf(version: string, env: Env): Promise<Snapshot | null> {
  const held = snapshots.get(version);
  if (held) return held;
  const object = await env.TILES.get(`${version}/${SNAPSHOT_KEY}`);
  if (object === null) return null;
  const listed = JSON.parse(await object.text()) as { base: string; written: string[]; removed: string[] };
  const snapshot = { base: listed.base, written: new Set(listed.written), removed: new Set(listed.removed) };
  snapshots.set(version, snapshot);
  return snapshot;
}

/** The key a page is read from, or null for a page its version removes. */
async function resolve(key: string, env: Env): Promise<string | null> {
  const match = BAKE_KEY.exec(key);
  if (!match) return key;
  const [, kind, version, name] = match;
  const snapshot = await snapshotOf(`${kind}/${version}`, env);
  if (snapshot === null || snapshot.written.has(name)) return key;
  return snapshot.removed.has(name) ? null : `${kind}/${snapshot.base}/${name}`;
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);

    // What the bucket answers, claimed first: the baked pages and the assets,
    // each under its own root. An asset's key *is* its path without the
    // leading slash — the root and the bucket's prefix are the same word —
    // where a page's key is its path with the tile root removed.
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
    // difference anyway (the kernel's layer/terrain/page.rs turns any 404
    // into the ocean tile, and layer/poi.rs into an empty page).
    if (!KEY_RE.test(key)) return new Response("not found", { status: 404 });

    // A missing object is not an error here: open-ocean pages are never
    // baked, and the client turns this 404 into its shared `_ocean` tile.
    // See the kernel's layer/terrain/page.rs.
    const cache = caches.default;
    if (request.method === "GET") {
      const cached = await cache.match(request);
      if (cached) return serve(cached.body, cached.headers);
    }
    const stored = await resolve(key, env);
    if (stored === null) return new Response(request.method === "HEAD" ? null : "not found", { status: 404 });

    if (request.method === "HEAD") {
      const meta = await env.TILES.head(stored);
      if (meta === null) return new Response(null, { status: 404 });
      const headers = new Headers();
      meta.writeHttpMetadata(headers);
      headers.set("etag", meta.httpEtag);
      return new Response(null, { headers });
    }

    // Edge cache in front of R2. Sound without any invalidation story because
    // every bake and asset is immutable and versioned by URL — a bake publishes
    // a new prefix rather than replacing bytes. This is also the only real cost control: R2 has no
    // spend limit, and class-B reads are the one meter that scales with
    // traffic, so every edge hit is a read that never happens.
    //
    // What is cached is the packed bytes, and the client is what unpacks them.
    // That is not what caching the response as it stands does: an entry whose
    // `content-encoding` the cache can read is one the cache normalizes, and
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
    const object = await env.TILES.get(stored);
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

    // Read whole before anything is answered, so what goes out is a body of
    // a known length. A stream carried no length, and one cut short — an R2
    // read that failed, a proxy restarted mid-body — ended as if it were
    // done: a browser stored the short or empty body under `immutable` for a
    // year, and the asset stayed blank through every reload (2026-09-28). A
    // body of a known length is one a client can tell was cut, and never
    // stores; an R2 read that fails here throws, and no 200 goes out at all.
    // It also leaves no stream to tee for the cache. The bytes are the
    // stored ones, packed as they lie, so the length is theirs: `serve`
    // sends them as they are. `encodeBody: "manual"` is not needed on the
    // cache's copy — there is no `content-encoding` for the runtime to act on.
    const bytes = await object.arrayBuffer();
    ctx.waitUntil(cache.put(request, new Response(bytes, { headers })));
    return serve(bytes, headers);
  },
};
