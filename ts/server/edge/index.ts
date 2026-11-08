// @pxl/edge/index.ts

/**
 * The Cloudflare Worker that serves the app, its baked pages and its assets,
 * and forwards search's own live API, all from one origin.
 *
 * The platform serves static assets before this script runs (wrangler.jsonc).
 * What reaches this script is the roots of spec/spec/route.json. `/tile/*` and
 * `/asset/*` come straight out of the R2 bucket. `/geo/*` and `/api/*` are
 * reverse-proxied as they are to GEO_ORIGIN and API_ORIGIN (infra/README.md),
 * so that they stay same-origin with the app.
 *
 * The platform types the Worker needs are declared inline.
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
  /** Workers' edge cache. `default` is the cache shared within one data
   * center. */
  interface CacheStorage {
    readonly default: Cache;
  }
}

/** The fetch handler's third argument. `waitUntil` keeps work alive after the
 * response is sent. */
type ExecutionContext = { waitUntil(p: Promise<unknown>): void };

export type Env = {
  /** The bucket: the baked pages, the assets and `source`. */
  BUCKET: R2Bucket;
  /** The static assets' binding, used only when no other route matches. */
  ASSETS: { fetch(request: Request): Promise<Response> };
  /** The live API's own origin (infra/README.md), from wrangler.jsonc's
   * `vars`. */
  API_ORIGIN: string;
  /** The geo server's origin, set the same way. */
  GEO_ORIGIN: string;
};

import {
  API_PREFIX,
  ASSET_PREFIX,
  GEO_PREFIX,
  TILE_PREFIX,
} from "./route.spec.ts";
import { BASE_LAYERS, TILE_OBJECT, TILE_SPHERE } from "./publication.spec.ts";

/**
 * The object keys this Worker asks R2 for, told apart by their shape alone.
 *
 * - `<kind>/<object>`: a baked page. The kind is one of `BASE_LAYERS`, and the
 *   object is laid out under it as spec/spec/bake/publication.json says
 *   (`TILE_OBJECT`). The Earth's picture lies whole beside its tiles
 *   (`TILE_SPHERE`).
 * - `asset/<sha256>.<ext>`: a model or an image, named by its bytes alone
 *   (pxl-tool's ingest_asset). A catalog names it by its path, and the
 *   shipped table maps the path to the key. A changed file gets a new name.
 * - `source`: the source's declaration (pxl-geo-bake source.rs), the one
 *   object that changes under its key.
 */
const SOURCE_KEY = "source";

const KINDS = BASE_LAYERS.join("|");
const KEY_RE = new RegExp(
  String.raw`^(?:(?:${KINDS})\/${TILE_OBJECT}|earth\/[^/]+\/${TILE_SPHERE}|asset\/[0-9a-f]{64}\.(?:glb|png|webp)|${SOURCE_KEY})$`,
);

/** The header in which a cached entry keeps the encoding of its bytes. It
 * holds the entry's `content-encoding` under a name of our own, which the
 * cache does not normalize. */
const CACHED_ENCODING = "pxl-content-encoding";

/** Sends an entry to the client. The encoding goes back where HTTP expects
 * it, and the body is declared final, so that nothing encodes it a second
 * time.
 *
 * Without `encodeBody: "manual"`, the runtime compresses the already packed
 * body again, and the client's decoding then fails far from here.
 *
 * A client that cannot read the encoding cannot be refused here: Cloudflare
 * rewrites `accept-encoding` on the request a Worker is handed, and
 * negotiates with the real client itself on the way out. */
function serve(
  body: ArrayBuffer | ReadableStream | null,
  from: Headers,
): Response {
  const headers = new Headers(from);
  const encoding = headers.get(CACHED_ENCODING);
  if (encoding) {
    headers.delete(CACHED_ENCODING);
    headers.set("content-encoding", encoding);
  }
  return new Response(body, { headers, encodeBody: "manual" });
}

/** The answer for a page its version leaves out: a 204, as the tile servers
 * answer an empty tile. */
function left(): Response {
  return new Response(null, { status: 204 });
}

/**
 * The declaration changes under its one key, so no copy of it is kept at the
 * edge: every boot revalidates it against R2. A client whose copy is current
 * gets 304.
 */
async function declaration(request: Request, env: Env): Promise<Response> {
  const object = await env.BUCKET.get(SOURCE_KEY, { onlyIf: request.headers });
  if (object === null) return new Response("not found", { status: 404 });
  const headers = new Headers();
  object.writeHttpMetadata(headers);
  headers.set("etag", object.httpEtag);
  // When the precondition fails, R2 sends no body, and the client already
  // holds the current copy.
  if (!("body" in object)) return new Response(null, { status: 304, headers });
  // The body is read whole so that its length is known, as `fetch` below
  // reads a page.
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

    // What the bucket answers is claimed first: the baked pages and the
    // assets, each under its own root. An asset's key is its path without the
    // leading slash, because the route's root and the bucket's prefix are the
    // same word. A page's key is its path with the tile root removed.
    const bucket = url.pathname.startsWith(TILE_PREFIX)
      ? url.pathname.slice(TILE_PREFIX.length)
      : url.pathname.startsWith(ASSET_PREFIX)
        ? url.pathname.slice(1)
        : null;

    // Everything else under a claimed root is reverse-proxied to that root's
    // origin, at the same path, with the host header rewritten for Caddy.
    // Neither branch reads the path past its prefix.
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
    // A key of no known shape (KEY_RE) is refused before R2 is asked.
    if (!KEY_RE.test(key)) return new Response("not found", { status: 404 });
    if (key === SOURCE_KEY) return declaration(request, env);

    // A page that its version holds no object for is one its bake left out,
    // such as open sea, or a child that its parent already describes. It is
    // answered empty, never as not found. A 404 is kept for a key that names
    // nothing, an asset's included.
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
      const meta = await env.BUCKET.head(key);
      if (meta === null) return missing();
      const headers = new Headers();
      meta.writeHttpMetadata(headers);
      headers.set("etag", meta.httpEtag);
      return new Response(null, { headers });
    }

    // The edge cache sits in front of R2 with no invalidation, because every
    // bake and asset is immutable and versioned by its URL.
    //
    // The cache holds the packed bytes, and the client unpacks them. The cache
    // stores decoded an entry whose `content-encoding` it can read, and never
    // compresses it again (`application/octet-stream`). So `CACHED_ENCODING`
    // holds the encoding while the entry is stored, and both paths out put it
    // back on the way to the client.
    const object = await env.BUCKET.get(key);
    // A missing object is not cached: a version asked for before it is
    // uploaded would otherwise answer empty for good.
    if (object === null) return missing();

    // writeHttpMetadata replays the headers set at upload time, among them the
    // `content-encoding` the object was packed in.
    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);

    // `no-transform` keeps anything from re-encoding what is already in its
    // final encoding, on the way out or out of the cache.
    const cc = headers.get("cache-control");
    if (cc && !cc.includes("no-transform")) {
      headers.set("cache-control", `${cc}, no-transform`);
    }

    const encoding = headers.get("content-encoding");
    if (encoding) {
      headers.delete("content-encoding");
      headers.set(CACHED_ENCODING, encoding);
    }

    // The body is read whole before anything is answered, so that what goes
    // out has a known length. A client can then tell that a body was cut
    // short, and never stores it under `immutable`. An R2 read that fails here
    // throws, and no 200 goes out at all.
    const bytes = await object.arrayBuffer();
    ctx.waitUntil(cache.put(request, new Response(bytes, { headers })));
    return serve(bytes, headers);
  },
};
