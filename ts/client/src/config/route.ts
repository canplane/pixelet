// @pxl/client/config/route.ts

// By path, not alias: vite.config.ts imports this and its bundler knows no aliases.
import type { BaseLayer } from "./published.spec.ts";

import { API_PREFIX, ASSET_PREFIX, GEO_PREFIX, TILE_PREFIX, TILE_ROOT } from "./route.spec.ts";

/** `<tile>/<layer>/<version>/<name>`: the object a bake's number is, under
 * the version its source declares serving it (namespace/tile.ts). */
export function tileUrl(layer: BaseLayer, object: string): string {
  return `${TILE_ROOT}/${layer}/${object}`;
}

/** The source's declaration: the bucket's `source`, or a dev tile server's. */
export const TILE_SOURCE_ROUTE = `${TILE_ROOT}/source`;

/** The route root and the bucket prefix are the same word on purpose: the
 * URL is the key with a leading slash. Content-addressed, so unversioned. */
export function assetUrl(key: string): string {
  return `/${key}`;
}

/**
 * Prefixes a dev server forwards, split as the Worker splits them: `tiles` to
 * the tile server, the rest to the search server. `published` ones go to the
 * published deployment under `dev`.
 */
export const DEV_PROXY_ROUTES = [
  { path: TILE_PREFIX, tiles: true, published: true },
  { path: ASSET_PREFIX, tiles: true, published: true },
  { path: GEO_PREFIX, published: true },
  { path: API_PREFIX, app: true, published: true },
] as const satisfies readonly { path: string; tiles?: boolean; app?: boolean; published?: boolean }[];
