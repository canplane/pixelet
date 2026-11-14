// @pxl/client/service/origin/route.ts

import * as origin from "@/service/platform/origin.ts";
import { type BASE_KINDS } from "@/abi/publication.spec.ts";
import { SITE_URL, TILE_ROOT } from "./route.spec.ts";

/** The URL of a bake's object, `<tile root>/<layer>/<version>/<name>`. The
 * object carries the version the source declares for it
 * (service/sys/fs/tile.ts). */
export function tile_url(
  layer: (typeof BASE_KINDS)[number],
  object: string,
): string {
  return `${TILE_ROOT}/${layer}/${object}`;
}

/** The route of the source's declaration: the bucket's `source` object, or a
 * dev tile server's answer. */
export const TILE_SOURCE_ROUTE = `${TILE_ROOT}/source`;

export function href(route: string): string {
  return origin.remote ? `${SITE_URL}${route}` : route;
}

export function get(route: string, signal?: AbortSignal): Promise<Response> {
  return origin.get(href(route), signal);
}

/** An asset's URL is its key with a leading slash, because the route's root
 * and the bucket's prefix are the same word on purpose. Assets are addressed
 * by their content, so their URLs carry no version. */
export function asset_url(key: string): string {
  return `/${key}`;
}
