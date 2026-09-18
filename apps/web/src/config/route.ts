// @pxl/web/config/route.ts

/** Every same-origin route the app requests, and how a dev server forwards them. */

import { type BakeKind, PUBLISHED_VERSION } from "@pxl/shared/bake/published.ts";

export { PUBLISHED_VERSION };
export type TileKind = BakeKind;

/** Everything the server answers. The app routes on paths of its own, so a
 * relay tells a deep link from a server request by this root alone. */
const API_ROOT = "/api";

/** Bucket-served bakes, a branch under the same root. */
const TILE_ROOT = `${API_ROOT}/tiles`;

/** `<root>/<kind>[/<version>]`, joined with a page index by the caller. */
export function tile_base(kind: TileKind, published: boolean): string {
  return published ? `${TILE_ROOT}/${kind}/${PUBLISHED_VERSION[kind]}` : `${TILE_ROOT}/${kind}`;
}

/** A content-addressed asset — a model, a texture — by its bucket key
 * (`asset/<sha256>`, the key the asset ingest gave its bytes). No
 * version: the key is the content. Same path published or not, since the
 * dev API serves the ingest's output under it. */
export function asset_url(key: string): string {
  return `${TILE_ROOT}/${key}`;
}

export const SEARCH_URL = `${API_ROOT}/search`;

/** Mirrored as literals in worker/index.ts, which is bundled for the Workers
 * runtime rather than from src/. */
export const EDGE_TILE_PREFIX = `${TILE_ROOT}/`;
export const EDGE_API_PREFIX = `${API_ROOT}/`;

/**
 * Prefixes a dev server forwards. Paths rather than URLs: a deployment serves
 * all of them itself, and nothing is rewritten in transit.
 *
 * Two entries, and the split is the one the edge already makes: the Worker
 * sends `/api/tiles/*` to the bucket and the rest of `/api/*` to the API's
 * origin, so a dev server sending the same prefix to the tile server and the
 * rest to the search server is that arrangement with the bucket's side baked
 * on demand. `tiles` says which — a relay reading a prefix, never a route.
 *
 * `published` marks what a published deployment answers, so `dev:remote` can
 * forward it there and leave the local process out of the loop entirely.
 */
export const DEV_PROXY_ROUTES = [
  { path: EDGE_TILE_PREFIX, tiles: true, published: true },
  { path: API_ROOT, published: true },
] as const satisfies readonly { path: string; tiles?: boolean; published?: boolean }[];

