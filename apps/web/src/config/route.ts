// @pxl/web/config/route.ts

// By path, not alias: vite.config.ts imports this and its bundler knows no aliases.
import { type BakeKind, PUBLISHED_VERSION } from "./published.spec.ts";

import { API_PREFIX, ASSET_PREFIX, GEO_PREFIX, TILE_PREFIX, TILE_ROOT } from "./route.spec.ts";


/** `<tile>/<kind>[/<version>]/<name>`: the kernel names the object, the
 * version is a deployment fact (a dev server bakes on demand, unversioned). */
export function tileUrl(kind: BakeKind, name: string, published: boolean): string {
  return published ? `${TILE_ROOT}/${kind}/${PUBLISHED_VERSION[kind]}/${name}` : `${TILE_ROOT}/${kind}/${name}`;
}

/** A dev tile server's declaration; a published one is in the bundle (`PUBLISHED_SOURCE`). */
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
