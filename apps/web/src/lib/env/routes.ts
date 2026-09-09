// @pxl/web/lib/env/routes.ts

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

export const SEARCH_URL = `${API_ROOT}/search`;

/**
 * Prefixes a dev server forwards, and which origin each comes from. Paths
 * rather than URLs: a deployment serves all of them itself.
 *
 * Order matters — the tile prefix sits inside the API root and has to be
 * tested first. Nothing is rewritten in transit.
 */
export const DEV_PROXY_ROUTES = [
  { path: TILE_ROOT, origin: "published" },
  { path: API_ROOT, origin: "live" },
  { path: IO_PATH, origin: "live", ws: true },
] as const satisfies readonly {
  path: string;
  origin: "published" | "live";
  ws?: boolean;
}[];

/** Mirrored as literals in worker/index.ts, which is bundled for the Workers
 * runtime rather than from src/. */
export const EDGE_TILE_PREFIX = `${TILE_ROOT}/`;
export const EDGE_API_PREFIX = `${API_ROOT}/`;
