// @pxl/client/origin/route.ts

import * as origin from "@/service/origin.ts";
import type { BaseLayer } from "@/origin/publication.spec.ts";
import { TILE_ROOT } from "./route.spec.ts";

/** `<tile>/<layer>/<version>/<name>`: the object a bake's name is, under
 * the version its source declares serving it (namespace/tile.ts). */
export function tile_url(layer: BaseLayer, object: string): string {
  return `${TILE_ROOT}/${layer}/${object}`;
}

/** The source's declaration: the bucket's `source`, or a dev tile server's. */
export const TILE_SOURCE_ROUTE = `${TILE_ROOT}/source`;

/** A route as the platform reads it: a page's own, a device's on the site. */
export function href(route: string): string {
  return `${origin.url}${route}`;
}

/** A route read from the origin, as the platform fetches (service/origin.ts). */
export const get = origin.get;

/** The route root and the bucket prefix are the same word on purpose: the
 * URL is the key with a leading slash. Content-addressed, so unversioned. */
export function asset_url(key: string): string {
  return `/${key}`;
}
