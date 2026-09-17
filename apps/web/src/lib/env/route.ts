// @pxl/web/lib/env/route.ts

/** Every same-origin route the app requests, and how a dev server forwards them. */

import { type BakeKind, PUBLISHED_VERSION } from "@pxl/shared/bake/published.ts";

export { PUBLISHED_VERSION };
export type TileKind = BakeKind;

/** Everything the server answers. The app routes on paths of its own, so a
 * relay tells a deep link from a server request by this root alone. */
const API_ROOT = "/api";

/** Bucket-served bakes, a branch under the same root. */
const TILE_ROOT = `${API_ROOT}/tiles`;

/** Socket upgrade path, from the API's own SOCKET_PATH. The production edge
 * does not proxy it. */
const IO_PATH = "/io";

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

/**
 * Prefixes a dev server forwards. Paths rather than URLs: a deployment
 * serves all of them itself, and nothing is rewritten in transit.
 *
 * The API root needs no tile entry beside it — tiles are a branch under it
 * and forward with it, whichever origin it points at.
 *
 * `local_only` marks what a published deployment cannot answer, so those
 * routes stay on the local API even in remote mode.
 */
export const DEV_PROXY_ROUTES = [
  { path: API_ROOT },
  { path: IO_PATH, local_only: true, ws: true },
] as const satisfies readonly {
  path: string;
  local_only?: boolean;
  ws?: boolean;
}[];

/** Mirrored as literals in worker/index.ts, which is bundled for the Workers
 * runtime rather than from src/. */
export const EDGE_TILE_PREFIX = `${TILE_ROOT}/`;
export const EDGE_API_PREFIX = `${API_ROOT}/`;
