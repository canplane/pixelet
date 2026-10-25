// @pxl/client/namespace/route.ts

import type { BaseLayer } from "@/namespace/published.spec.ts";
import { TILE_ROOT } from "@/config/route.spec.ts";

/** `<tile>/<layer>/<version>/<name>`: the object a bake's number is, under
 * the version its source declares serving it (namespace/tile.ts). */
export function tile_url(layer: BaseLayer, object: string): string {
  return `${TILE_ROOT}/${layer}/${object}`;
}

/** The source's declaration: the bucket's `source`, or a dev tile server's. */
export const TILE_SOURCE_ROUTE = `${TILE_ROOT}/source`;

/** The route root and the bucket prefix are the same word on purpose: the
 * URL is the key with a leading slash. Content-addressed, so unversioned. */
export function asset_url(key: string): string {
  return `/${key}`;
}
