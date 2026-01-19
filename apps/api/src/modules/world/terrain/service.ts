// @pxl/api/modules/world/terrain/service.ts

import { get_xpute } from "@/xpute/context.ts";
import type { AppResources } from "@/app/resources.ts";

import type { f64, i16, u32 } from "@xpute/spec/lib/kit/word.ts";
import type { Bytes, I16Array } from "@xpute/spec/lib/kit/array.ts";
import { to_tuple } from "@xpute/spec/lib/kit/lower.ts";
import { encoder, TreeView } from "@xpute/spec/io/marshal/tree/mod.ts";

import type { BBox3857 } from "@pxl/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/spec/space/adapter/3857.ts";
import { type IVec, ivec } from "@pxl/spec/space/pvf3/mod.ts";

import { Command } from "@xpute/spec/control/cmd.ts";
import { COAST_ROW_KEYS, type CoastRow, FEAT_ROW_KEYS, type FeatRow } from "@pxl/bake/terrain/format.ts";
import { ELE_PADDED_SPAN } from "@pxl/bake/terrain/constants.ts";
import { TerrainRepository } from "./repository.ts";

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
      console.error(`[${Command.TERRAIN_SOURCE}/${ivec.repr(page_idx)}] shell: db error:`, err);
      throw err;
    }

    let ele: I16Array;
    if (_ele && _ele.length === ELE_PADDED_SPAN2) {
      ele = new Int16Array(_ele);
    } else {
      console.warn(`[${Command.TERRAIN_SOURCE}/${ivec.repr(page_idx)}] shell: invalid ele shape`, _ele?.length ?? null);
      ele = new Int16Array(ELE_PADDED_SPAN2); // fallback
    }

    console.debug(
      `[${Command.TERRAIN_SOURCE}/${ivec.repr(page_idx)}] shell: ele=${ele.length}, feat_rows=${feat_rows.length}, coast_rows=${coast_rows.length}`,
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

  return { fetch_terrain_source };
}

export function register(resrc: AppResources) {
  const { post } = get_xpute();

  const repo = new TerrainRepository(resrc.geo_pool);
  const service = make_services(repo);

  const unsub = post.on<[IVec]>(Command.TERRAIN_SOURCE, (args, _body) => {
    const [page_idx] = args;

    return service.fetch_terrain_source(page_idx);
  });

  resrc.disposers.push(unsub);
}
