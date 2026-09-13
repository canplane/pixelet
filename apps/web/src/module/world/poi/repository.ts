// @pxl/web/module/world/poi/repository.ts

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { Block } from "@xpute/core/ds/buddy.ts";
import { encode_empty_poi_page, PoiPage } from "@pxl/shared/bake/format.ts";

import { PAGE_POOL } from "@/render/layer/pipeline/core_pool.ts";

import { POI_TILE_BASE_URL, PUBLISHED_TILES } from "@/lib/env/config.ts";

/**
 * POI transport: the pre-baked static set when one is configured
 * (apps/api/src/bake/poi/tool/bake_poi_tiles.ts), the live API otherwise —
 * same PUBLISHED_TILES shape terrain/tile_repository.ts uses, one page
 * simpler: a missing object just means "no POI here", not "open ocean,
 * resolve to a shared fallback tile". There is no fallback object to fetch,
 * so a 404 (static) resolves straight to `[]` with no extra round trip.
 *
 * Static and live are supposed to agree byte-for-semantics (both go through
 * provider/overture_provider.ts's own PoiProvider — see provider.ts), so
 * which one serves a given request is purely a deployment concern, not
 * something either the client or the bake tool needs to reconcile on its
 * own.
 */
function poi_url(key: string): string {
  return `${POI_TILE_BASE_URL}/${key}`;
}

/**
 * A resident POI page: the packet copied into the core memory, and the page
 * opened over it. The lanes the index reads are views into that block, so
 * the block lives exactly as long as the page is resident (poi/store.ts
 * releases it with the page). The empty page — a 404 from the static bake,
 * or a page with nothing on it — is one shared packet nothing releases.
 */
export type ResidentPoiPage = { page: PoiPage; block: Block | null };

const EMPTY_PAGE: ResidentPoiPage = { page: new PoiPage(encode_empty_poi_page()), block: null };

function into_core(pkt: Uint8Array): ResidentPoiPage {
  const block = PAGE_POOL.acquire(pkt.byteLength);
  const view = new Uint8Array(block.buf, block.byte_offset, pkt.byteLength);
  view.set(pkt);
  return { page: new PoiPage(view), block };
}

export function release_poi_page(resident: ResidentPoiPage): void {
  if (resident.block) PAGE_POOL.release(resident.block);
}

/** `phases`, when given, is filled with this page's network-vs-open split —
 * same reasoning as tile_repository.ts's own. Opening is a header read and
 * a handful of views; what used to be `JSON.parse` over thousands of
 * records is gone, and the strings are read when a badge or a detail view
 * asks for one. */
export async function fetch_poi(
  page_idx: IVec,
  signal?: AbortSignal,
  phases?: Record<string, number>,
): Promise<ResidentPoiPage> {
  const t0 = performance.now();
  const res = await fetch(poi_url(page_idx.toString()), { signal });
  if (res.status === 404 && PUBLISHED_TILES) return EMPTY_PAGE;
  if (!res.ok) throw new Error(`[poi/${ivec.repr(page_idx)}] http ${res.status}`);

  const bytes = new Uint8Array(await res.arrayBuffer());
  const t1 = performance.now();
  const resident = into_core(bytes);

  if (phases) {
    phases.net_ms = t1 - t0;
    phases.parse_ms = performance.now() - t1;
    phases.bytes = bytes.byteLength;
  }
  return resident;
}
