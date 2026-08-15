// @pxl/api/modules/world/terrain/service.ts

import type { Hono } from "@hono/hono";

import type { AppResources } from "@/app/resources.ts";

import type { f64, i16, u32 } from "@xpute/core/kit/word.ts";
import type { Bytes, I16Array } from "@xpute/core/kit/array.ts";
import { to_tuple } from "@xpute/core/kit/lower.ts";
import { encoder, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";

import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { IVEC_WORD_MASK } from "@pxl/shared/spec/space/pvf3/core.ts";

import { COAST_ROW_KEYS, type CoastRow, FEAT_ROW_KEYS, type FeatRow } from "@pxl/shared/bake/format.ts";
import { ELE_PADDED_SPAN } from "@pxl/shared/bake/constants.ts";
import { L3_SHAMT } from "@pxl/shared/spec/space/indexer.ts";
import { bake_terrain } from "@pxl/api/bake/terrain/bake.ts";
import { TerrainRepository } from "./repository.ts";

const MAX_L3_IDX = IVEC_WORD_MASK >> BigInt(L3_SHAMT);

/**
 * Digits only, capped well above MAX_L3_IDX's own ~18-digit width (2^57 ≈
 * 1.44e17) but far short of a string long enough to make BigInt()'s own
 * O(n^2)-ish decimal parse expensive — checked *before* BigInt() ever
 * touches attacker-controlled input, so an oversized digit string is
 * rejected as cheaply as a malformed one instead of paying to parse it
 * first. BigInt()'s constructor also accepts "-", "+", "0x…"/"0b…"/"0o…",
 * and surrounding whitespace, any of which could still parse to a value
 * outside page_idx's real domain below — this regex is stricter than
 * "parses as BigInt", not just a length guard.
 */
const PAGE_IDX_PATTERN = /^\d{1,20}$/;

const ELE_PADDED_SPAN2: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN;

function make_services(repo: TerrainRepository) {
  const fetch_terrain_source = async (page_idx: IVec): Promise<Bytes> => {
    const page_bbox: BBox3857 = epsg3857.get_page_bbox(page_idx);
    const page_z_scale: f64 = epsg3857.Z_SCALE(page_bbox[0][1]);

    // DB fetch
    let _ele: i16[] | null;
    let feat_rows: FeatRow[];
    let coast_rows: CoastRow[];
    try {
      [_ele, feat_rows, coast_rows] = await Promise.all([
        repo.fetch_ele(page_bbox, page_z_scale),
        repo.fetch_feat_rows(page_bbox),
        repo.fetch_coast_rows(page_bbox),
      ]);
    } catch (err) {
      console.error(`[terrain/source/${ivec.repr(page_idx)}] db error:`, err);
      throw err;
    }

    let ele: I16Array;
    if (_ele && _ele.length === ELE_PADDED_SPAN2) {
      ele = new Int16Array(_ele);
    } else {
      console.warn(`[terrain/source/${ivec.repr(page_idx)}] invalid ele shape`, _ele?.length ?? null);
      ele = new Int16Array(ELE_PADDED_SPAN2); // fallback
    }

    console.debug(
      `[terrain/source/${ivec.repr(page_idx)}] ele=${ele.length}, feat_rows=${feat_rows.length}, coast_rows=${coast_rows.length}`,
    );

    const feats_view = new TreeView().branch((__) => {
      for (const row of feat_rows) {
        __.put(to_tuple(row, FEAT_ROW_KEYS));
      }
    });
    const coasts_view = new TreeView().branch((__) => {
      for (const row of coast_rows) {
        __.put(to_tuple(row, COAST_ROW_KEYS));
      }
    });

    return encoder.encode(new TreeView().branch((__) =>
      __.i16_array(ele)
        .subtree(feats_view)
        .subtree(coasts_view)
    ));
  };

  // Raw OSM/elevation source, baked into client-ready ground channels.
  // This step used to run inside the (now-dropped) WASM worker.
  const fetch_tile = async (page_idx: IVec): Promise<Bytes> => {
    const raw: Bytes = await fetch_terrain_source(page_idx);
    return bake_terrain(page_idx, raw);
  };

  return { fetch_terrain_source, fetch_tile };
}

export function register(router: Hono, resrc: AppResources): void {
  const repo = new TerrainRepository(resrc.geo_pool);
  const service = make_services(repo);

  router.get("/api/world/terrain/tile/:page_idx", async (c) => {
    const raw_page_idx: string = c.req.param("page_idx");

    // page_idx is an L3_IDX (see indexer.ts): an *unsigned* logical key in
    // [0, MAX_L3_IDX]. A negative or out-of-range value isn't just
    // "invalid" the way a NaN would be — FROM_L3_IDX/get_page_bbox happily
    // shift and unproject it into some nonsense-but-well-formed bbox with
    // no exception anywhere along the way, which would've reached the DB
    // as a garbage query instead of failing fast here.
    if (!PAGE_IDX_PATTERN.test(raw_page_idx)) {
      return c.text("bad page_idx", 400);
    }

    const page_idx: IVec = BigInt(raw_page_idx);
    if (page_idx > MAX_L3_IDX) {
      return c.text("bad page_idx", 400);
    }

    const bytes: Bytes = await service.fetch_tile(page_idx);

    // hono's own compress() middleware (main.ts) skips this response: it
    // gates on Content-Type, and application/octet-stream isn't in its
    // compressible-type allowlist (reasonably so in general — opaque binary
    // is often already-compressed media). This payload isn't that: it's
    // dense per-cell ground-channel bytes built from polygon fills, mostly
    // large uniform runs (see format.ts's encode_terrain) — gzips well.
    // fetch() on the client decodes Content-Encoding transparently, so this
    // needs no client-side change.
    if ((c.req.header("accept-encoding") ?? "").includes("gzip")) {
      const gzip_body = new Response(bytes).body!.pipeThrough(new CompressionStream("gzip"));
      return new Response(gzip_body, {
        status: 200,
        headers: { "content-type": "application/octet-stream", "content-encoding": "gzip" },
      });
    }

    return c.body(bytes, 200, { "content-type": "application/octet-stream" });
  });
}
