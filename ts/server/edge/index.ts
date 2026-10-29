// @pxl/edge/index.ts

/**
 * Cloudflare Worker that serves the app, its baked pages and assets, and
 * forward search's own live API — all from one origin.
 *
 * Static assets are handled by the platform before this script runs (see
 * wrangler.jsonc). What reaches this script is the roots of
 * spec/spec/route.json: `/tile/*` and `/asset/*` come straight out of the
 * R2 bucket, and `/geo/*` and `/api/*` are reverse-proxied to GEO_ORIGIN and API_ORIGIN as-is (see infra/README.md).
 * Keeping it on the app's own hostname is what makes them same-origin, so no
 * CORS policy exists to configure or get wrong.
 *
 * A package of its own beside the servers (ts/server/edge): the only
 * server-side code in the deployment, for the Workers runtime rather than
 * the browser. The handful of platform types it needs are declared inline
 * rather than pulling in @cloudflare/workers-types for one file.
 */

type R2Object = {
  body: ReadableStream;
  arrayBuffer(): Promise<ArrayBuffer>;
  text(): Promise<string>;
  httpEtag: string;
  writeHttpMetadata(headers: Headers): void;
};
/** What R2 answers without the body: a `head`, or a `get` whose precondition failed. */
type R2Head = Pick<R2Object, "httpEtag" | "writeHttpMetadata">;
type R2Bucket = {
  get(key: string): Promise<R2Object | null>;
  get(
    key: string,
    options: { onlyIf: Headers },
  ): Promise<R2Object | R2Head | null>;
  head(key: string): Promise<R2Head | null>;
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

import {
  API_PREFIX,
  ASSET_PREFIX,
  GEO_PREFIX,
  TILE_OBJECT,
  TILE_PREFIX,
  TILE_SPHERE,
} from "./route.spec.ts";
import { BASE_LAYERS } from "./published.spec.ts";

/**
 * Object keys this Worker is willing to ask R2 for, by shape alone, so what
 * was never an address does not reach the bucket; what bounds the reads is
 * the free plan's daily cap.
 *
 * - `<kind>/<object>`: a baked page, a kind of `BASE_LAYERS` and the object
 *   spec/spec/route.json shapes under it (`TILE_OBJECT`), and the Earth's
 *   picture whole beside its tiles (`TILE_SPHERE`). Which versions and
 *   names exist is the declaration's and the tiles' to say, not this file's.
 * - `asset/<sha256>.<ext>`: a model or an image named by its bytes alone
 *   (pxl-tool's ingest_asset); a catalog names it by its path and the shipped
 *   table maps that to the key. A changed file is a new name, which is what
 *   makes `immutable` true of it.
 * - `source`: the source's declaration (pxl-geo-bake source.rs), the one
 *   object that changes under its key.
 */
const SOURCE_KEY = "source";

const KINDS = BASE_LAYERS.join("|");
const KEY_RE = new RegExp(
  String.raw`^(?:(?:${KINDS})\/${TILE_OBJECT}|earth\/[^/]+\/${TILE_SPHERE}|asset\/[0-9a-f]{64}\.(?:glb|png|webp)|${SOURCE_KEY})$`,
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
function serve(
  body: ArrayBuffer | ReadableStream | null,
  from: Headers,
): Response {
  const headers = new Headers(from);
  const encoding = headers.get(STORED_ENCODING);
  if (encoding) {
    headers.delete(STORED_ENCODING);
    headers.set("content-encoding", encoding);
  }
  return new Response(body, { headers, encodeBody: "manual" });
}

/** A page its version leaves out, as the tile servers answer an empty tile. */
function left(): Response {
  return new Response(null, { status: 204 });
}

/**
 * The declaration changes under its one key, so no copy of it is kept at the
 * edge: every boot revalidates it against R2, a read the daily cap bounds, and
 * a publish is seen by the next boot. A client whose copy is current gets 304.
 */
async function declaration(request: Request, env: Env): Promise<Response> {
  const object = await env.TILES.get(SOURCE_KEY, { onlyIf: request.headers });
  if (object === null) return new Response("not found", { status: 404 });
  const headers = new Headers();
  object.writeHttpMetadata(headers);
  headers.set("etag", object.httpEtag);
  // A precondition that failed leaves no body: the client holds the current one.
  if (!("body" in object)) return new Response(null, { status: 304, headers });
  // Whole, so its length is known, as `fetch` below reads a page.
  return new Response(
    request.method === "HEAD" ? null : await object.arrayBuffer(),
    { headers },
  );
}

/** A baked page's key: a kind, a version and a page name. */
const BAKE_KEY = new RegExp(String.raw`^(?:${KINDS})\/`);

export default {
  async fetch(
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> {
    const url = new URL(request.url);

    // What the bucket answers, claimed first: the baked pages and the assets,
    // each under its own root. An asset's key *is* its path without the
    // leading slash — the root and the bucket's prefix are the same word —
    // where a page's key is its path with the tile root removed.
    const bucket = url.pathname.startsWith(TILE_PREFIX)
      ? url.pathname.slice(TILE_PREFIX.length)
      : url.pathname.startsWith(ASSET_PREFIX)
        ? url.pathname.slice(1)
        : null;

    // Everything else under a claimed root is reverse-proxied to that root's
    // origin, same path, host header rewritten for Caddy. Neither branch reads
    // past its prefix.
    if (bucket === null) {
      const origin = url.pathname.startsWith(GEO_PREFIX)
        ? env.GEO_ORIGIN
        : url.pathname.startsWith(API_PREFIX)
          ? env.API_ORIGIN
          : null;
      if (origin) {
        const target = new URL(url.pathname + url.search, origin);
        const headers = new Headers(request.headers);
        headers.set("host", target.host);
        return fetch(
          new Request(target, {
            method: request.method,
            headers,
            body: request.body,
          }),
        );
      }
      return env.ASSETS.fetch(request);
    }

    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response("method not allowed", {
        status: 405,
        headers: { allow: "GET, HEAD" },
      });
    }

    const key = bucket;
    // Rejected before R2 is touched — see KEY_RE.
    if (!KEY_RE.test(key)) return new Response("not found", { status: 404 });
    if (key === SOURCE_KEY) return declaration(request, env);

    // A page a version holds no object for is one its bake leaves out — open
    // sea, or a child its parent already says — and so is answered empty,
    // never not found; 404 stays for what names nothing, an asset included.
    const page = BAKE_KEY.test(key);
    const missing = () =>
      page
        ? left()
        : new Response(request.method === "HEAD" ? null : "not found", {
            status: 404,
          });
    const cache = caches.default;
    if (request.method === "GET") {
      const cached = await cache.match(request);
      if (cached) return serve(cached.body, cached.headers);
    }
    if (request.method === "HEAD") {
      const meta = await env.TILES.head(key);
      if (meta === null) return missing();
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
    const object = await env.TILES.get(key);
    // Not cached: a version asked for before it is uploaded would answer empty for good.
    if (object === null) return missing();

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
    if (cc && !cc.includes("no-transform")) {
      headers.set("cache-control", `${cc}, no-transform`);
    }

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
