// @pxl/web/modules/world/poi/repository.ts

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";

import { HAS_STATIC_POI, POI_TILE_BASE_URL } from "@/lib/env/config.ts";

/**
 * POI transport: the pre-baked static set when one is configured
 * (apps/api/src/bake/poi/tools/bake_poi_tiles.ts), the live API otherwise —
 * same HAS_STATIC_TILES shape terrain/tile_repository.ts uses, one page
 * simpler: a missing object just means "no POI here", not "open ocean,
 * resolve to a shared fallback tile". There is no fallback object to fetch,
 * so a 404 (static) resolves straight to `[]` with no extra round trip.
 *
 * Static and live are supposed to agree byte-for-semantics (both go through
 * providers/overture_provider.ts's own PoiProvider — see provider.ts), so
 * which one serves a given request is purely a deployment concern, not
 * something either the client or the bake tool needs to reconcile on its
 * own.
 */
function poi_url(key: string): string {
  return `${POI_TILE_BASE_URL}/${key}`;
}

/** `phases`, when given, is filled with this page's network-vs-JSON-parse
 * split — same reasoning as tile_repository.ts's own, and not academic here:
 * a dense urban page carries thousands of records, so `res.json()` is real
 * main-thread work, not a rounding error on the request. */
export async function fetch_poi(
  page_idx: IVec,
  signal?: AbortSignal,
  phases?: Record<string, number>,
): Promise<POI[]> {
  const t0 = performance.now();
  const res = await fetch(poi_url(page_idx.toString()), { signal });
  if (res.status === 404 && HAS_STATIC_POI) return [];
  if (!res.ok) throw new Error(`[poi/${ivec.repr(page_idx)}] http ${res.status}`);

  const text = await res.text();
  const t1 = performance.now();
  const pois = JSON.parse(text) as POI[];

  if (phases) {
    phases.net_ms = t1 - t0;
    phases.parse_ms = performance.now() - t1;
    phases.bytes = text.length;
  }
  return pois;
}
