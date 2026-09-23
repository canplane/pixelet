// @pxl/web/config/route.ts

/** Every same-origin route the app requests, and how a dev server forwards them. */

// A sibling by its own path, as route.spec.ts is below. The alias would be
// the house style, but vite.config.ts imports this module for the dev
// proxy's routes and its own bundler knows no aliases — under `@/` the
// import went unresolved and was left external, and Deno's import map is
// what quietly made it work.
import { PUBLISHED_VERSION } from "./published.spec.ts";

import { API_PREFIX, ASSET_PREFIX, GEO_PREFIX, TILE_PREFIX } from "./route.spec.ts";

export { PUBLISHED_VERSION };
export { API_PREFIX, API_ROOT, ASSET_PREFIX, ASSET_ROOT, GEO_PREFIX, GEO_ROOT, SEARCH_URL, TILE_PREFIX, TILE_ROOT } from "./route.spec.ts";

/** A content-addressed asset — a model, a texture — by its bucket key
 * (`asset/<sha256>`, what the ingest gave its bytes). The root and the bucket's
 * own prefix are the same word deliberately, so the URL is the key with a
 * leading slash and the edge's mapping back is that slash removed. No version:
 * the key is the content, so the URL is as immutable as one gets. */
export function asset_url(key: string): string {
  return `/${key}`;
}

/**
 * Prefixes a dev server forwards. Paths rather than URLs: a deployment serves
 * all of them itself, and nothing is rewritten in transit.
 *
 * The split is the edge's own. The Worker sends `/geo/tiles/` and `/asset/` to
 * the bucket and the rest of `/geo/` to the origin, so a dev server sending the
 * first two to the tile server and the rest to the search server is that
 * arrangement with the bucket's side baked on demand rather than read. `tiles`
 * says which — a relay reading a prefix, never a route.
 *
 * `published` marks what a published deployment answers, so `dev:remote` can
 * forward it there and leave the local processes out of the loop entirely.
 * `/api/` is not among them — pxl-api answers nothing yet, so a published
 * deployment has nothing there to forward to — but it is still proxied
 * locally, because the process is running and a 404 from it reads differently
 * from the app's own index.html coming back.
 */
export const DEV_PROXY_ROUTES = [
  { path: TILE_PREFIX, tiles: true, published: true },
  { path: ASSET_PREFIX, tiles: true, published: true },
  { path: GEO_PREFIX, published: true },
  { path: API_PREFIX, app: true },
] as const satisfies readonly { path: string; tiles?: boolean; app?: boolean; published?: boolean }[];
