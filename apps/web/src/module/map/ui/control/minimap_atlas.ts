// @pxl/web/module/map/ui/control/minimap_atlas.ts

import { useEffect, useRef, useState } from "react";

import type { f64, u32, u8 } from "@xpute/core/kit/word.ts";
import { clamp } from "@xpute/core/kit/math.ts";
import { type IVec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { FROM_L3_IDX, L3_CENTER, L3_IDX, L3_RANGE } from "@pxl/shared/spec/space/indexer.ts";
import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { PAGE_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import { cube_colors, cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { useMapStore } from "@/module/map/store.ts";
import { tile_pages, useTileStore } from "@/module/world/terrain/store.ts";
import { camera_cfg } from "@/module/camera/config.ts";

const { DEFAULT_DIST } = camera_cfg.view;

const ATLAS_PAGE_PX: u32 = 64;
const SAMPLE_STRIDE: u32 = PAGE_CELL_SPAN / ATLAS_PAGE_PX;
const ATLAS_RADIUS_PAGES: u32 = 2;

const L3_RANGE_REAL: f64 = Number(L3_RANGE);
const ATLAS_SPAN_PAGES: u32 = ATLAS_RADIUS_PAGES * 2 + 1;
const ATLAS_PX: u32 = ATLAS_SPAN_PAGES * ATLAS_PAGE_PX;
const ATLAS_CENTER_PX: f64 = ATLAS_PX / 2;

export const MINIMAP_PX_PER_U: f64 = ATLAS_PAGE_PX / L3_RANGE_REAL;

// Ground channel bytes are type IDs, not weights, so this must reuse the same
// per-channel override order the terrain paint resolves a cell by
// (crates/pxl-core/src/bake/terrain.rs) — reordering
// GROUND_CH_KEYS here would silently mismatch the 3D terrain's colors.
const COLOR_TABLE: Record<GroundChannelKey, Record<u8, string | null>> = cube_colors;
const BASE_COLOR: string = cube_colors.land[cube_types.land.BARE];

const $delta: PVF3 = { i: 0n, f: 0n };

function page_rel_u(page_idx: IVec, origin: PVF3): { x: f64; y: f64 } {
  const page_base: IVec = FROM_L3_IDX(page_idx);
  pvf3.sub($delta, { i: page_base, f: 0n }, origin);
  return { x: pvf3.to_real(pvf3.X($delta)), y: pvf3.to_real(pvf3.Y($delta)) };
}

export type MinimapAtlas = {
  url: string;
  origin: PVF3;
};

export function useMinimapAtlas(): MinimapAtlas | null {
  const tile_revision = useTileStore((state) => state.revision);
  const map_center = useMapStore((state) => state.map_center);

  const canvas_ref = useRef<HTMLCanvasElement | null>(null);
  if (!canvas_ref.current) {
    const canvas = document.createElement("canvas");
    canvas.width = ATLAS_PX;
    canvas.height = ATLAS_PX;
    canvas_ref.current = canvas;
  }

  const center_page_idx: IVec | null = map_center ? L3_IDX(map_center.i) : null;

  const [atlas, set_atlas] = useState<MinimapAtlas | null>(null);

  useEffect(() => {
    if (!map_center || center_page_idx === null) return;

    const origin: PVF3 = { i: L3_CENTER(map_center.i), f: 0n };

    const canvas = canvas_ref.current!;
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = BASE_COLOR;
    ctx.fillRect(0, 0, ATLAS_PX, ATLAS_PX);

    const max_rel_u: f64 = (ATLAS_RADIUS_PAGES + 0.5) * L3_RANGE_REAL;

    for (const [page_idx, tile] of tile_pages.entries()) {
      const rel = page_rel_u(page_idx, origin);
      if (Math.abs(rel.x) > max_rel_u || Math.abs(rel.y) > max_rel_u) continue;

      const px0: f64 = ATLAS_CENTER_PX + rel.x * MINIMAP_PX_PER_U;
      const py0: f64 = ATLAS_CENTER_PX - rel.y * MINIMAP_PX_PER_U - ATLAS_PAGE_PX;

      const ground = tile.terrain.ground;
      for (let sy: u32 = 0; sy < ATLAS_PAGE_PX; sy++) {
        // Source raster rows increase with world Y; atlas pixel rows increase
        // downward for a north-up image — flip, or the minimap renders upside down.
        const py: f64 = py0 + (ATLAS_PAGE_PX - 1 - sy);
        const cy: u32 = sy * SAMPLE_STRIDE;

        for (let sx: u32 = 0; sx < ATLAS_PAGE_PX; sx++) {
          const cx: u32 = sx * SAMPLE_STRIDE;
          const off: u32 = cy * PAGE_CELL_SPAN + cx;

          let color: string = BASE_COLOR;
          for (const ch_key of GROUND_CH_KEYS) {
            const v: u8 = ground[ch_key][off];
            if (v === 0) continue;
            const c = COLOR_TABLE[ch_key][v];
            if (c) color = c;
          }

          ctx.fillStyle = color;
          ctx.fillRect(px0 + sx, py, 1, 1);
        }
      }
    }

    set_atlas({ url: canvas.toDataURL(), origin });
    // map_center is deliberately excluded: it changes every pan tick and is handled
    // by minimap_pan_px() instead — including it here would rebake on every tick.
    // tile_revision: the pages walked above live in tile_pages.
  }, [tile_revision, center_page_idx]);

  return atlas;
}

export function minimap_pan_px(map_center: PVF3, origin: PVF3): { x: f64; y: f64 } {
  pvf3.sub($delta, map_center, origin);
  const dx: f64 = pvf3.to_real(pvf3.X($delta));
  const dy: f64 = pvf3.to_real(pvf3.Y($delta));
  return { x: -dx * MINIMAP_PX_PER_U, y: dy * MINIMAP_PX_PER_U };
}

const MIN_MINIMAP_SCALE: f64 = 0.5;
const MAX_MINIMAP_SCALE: f64 = 2.0;

export function minimap_scale_for_dist(dist: f64): f64 {
  return clamp(DEFAULT_DIST / dist, MIN_MINIMAP_SCALE, MAX_MINIMAP_SCALE);
}

export const MINIMAP_ATLAS_PX: u32 = ATLAS_PX;
