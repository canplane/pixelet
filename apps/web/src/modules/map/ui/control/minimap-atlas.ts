// @pxl/web/modules/map/ui/control/minimap-atlas.ts

/**
 * NOTE: not currently wired into the compass UI (disabled — value vs. screen space was
 * unclear without POI markers on it yet). Kept as-is since the same "bake loaded ground
 * channels into a north-up raster" logic is the right starting point for a map-export
 * feature; revisit wiring this back into MapCompass once POI rendering exists.
 */

import { useEffect, useRef, useState } from "react";

import type { f64, u32, u8 } from "@xpute/core/kit/word.ts";
import { clamp } from "@xpute/core/kit/math.ts";
import { type IVec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { FROM_L3_IDX, L3_CENTER, L3_IDX, L3_RANGE } from "@pxl/shared/spec/space/indexer.ts";
import { GROUND_CH_KEYS, type GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { PAGE_CELL_SPAN } from "@pxl/shared/bake/constants.ts";
import { terrain_colors, terrain_types } from "@pxl/shared/spec/cell/terrain.ts";

import { useMapStore } from "@/modules/map/store.ts";
import { useTileStore } from "@/modules/world/terrain/tile-store.ts";
import { camera_cfg } from "@/modules/camera/config.ts";

const { DEFAULT_DIST } = camera_cfg.view;

/** Pixels drawn per one L3 page width — tunes minimap zoom level. */
const ATLAS_PAGE_PX: u32 = 64;
/** Cell stride when downsampling a page's PAGE_CELL_SPAN x PAGE_CELL_SPAN ground raster. */
const SAMPLE_STRIDE: u32 = PAGE_CELL_SPAN / ATLAS_PAGE_PX;
/** Pages beyond this radius (in page widths) from the player aren't baked — bounds atlas size. */
const ATLAS_RADIUS_PAGES: u32 = 2;

const L3_RANGE_REAL: f64 = Number(L3_RANGE);
const ATLAS_SPAN_PAGES: u32 = ATLAS_RADIUS_PAGES * 2 + 1;
const ATLAS_PX: u32 = ATLAS_SPAN_PAGES * ATLAS_PAGE_PX;
const ATLAS_CENTER_PX: f64 = ATLAS_PX / 2;

/** Pixels drawn per canonical world unit ('u') — shared with the pan-offset math below. */
export const MINIMAP_PX_PER_U: f64 = ATLAS_PAGE_PX / L3_RANGE_REAL;

// Ground channel bytes are type IDs (e.g. land.GREEN_DEEP=10 for forest/mountain),
// not weights — so "biggest byte wins" is meaningless across channels. Reuse the exact
// same per-type palette and override order the 3D shader uses (see shader.ts's
// ch_blending_logic: land, then water, then structure, then transport, each overriding
// the previous if present), so the minimap actually distinguishes forest/park/sand/
// building types instead of flattening everything non-water into one cream blob.
const COLOR_TABLE: Record<GroundChannelKey, Record<u8, string | null>> = terrain_colors;
const BASE_COLOR: string = terrain_colors.land[terrain_types.land.BARE];

const $delta: PVF3 = { i: 0n, f: 0n };

/** World-relative (x, y) of a page's south-west corner, in real 'u' units from `origin`. */
function page_rel_u(page_idx: IVec, origin: PVF3): { x: f64; y: f64 } {
  const page_base: IVec = FROM_L3_IDX(page_idx);
  pvf3.sub($delta, { i: page_base, f: 0n }, origin);
  return { x: pvf3.to_real(pvf3.X($delta)), y: pvf3.to_real(pvf3.Y($delta)) };
}

export type MinimapAtlas = {
  /** Rendered atlas as a data URL — cheap to hand to a plain CSS background-image. */
  url: string;
  /** The world point the atlas was centered on when baked; pan math is relative to this. */
  origin: PVF3;
};

/**
 * Bakes loaded terrain pages into a small offscreen "north-up" raster, flat-shaded
 * by dominant ground channel per cell. A real 3D re-render for the minimap would cost
 * a second scene/camera pass every frame for no visual gain at this size, so this reads
 * straight from the already-fetched ground channels instead — the same data the 3D
 * terrain splatmaps use (see terrain/chunk.tsx), just downsampled and flattened to 2D.
 *
 * Only rebuilds when the loaded tile set changes or the player crosses into a different
 * L3 page (both captured by the effect's dependency array — no manual bookkeeping
 * needed). Panning *within* an already-baked atlas is pure CSS transform math computed
 * from `origin` (see `minimap_pan_px`), not a redraw.
 */
export function useMinimapAtlas(): MinimapAtlas | null {
  const tile_map = useTileStore((state) => state.tile_map);
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

    for (const [page_idx, tile] of tile_map) {
      const rel = page_rel_u(page_idx, origin);
      if (Math.abs(rel.x) > max_rel_u || Math.abs(rel.y) > max_rel_u) continue;

      // px0/py0 = pixel position of this page's north-west corner (min-x, max-y).
      const px0: f64 = ATLAS_CENTER_PX + rel.x * MINIMAP_PX_PER_U;
      const py0: f64 = ATLAS_CENTER_PX - rel.y * MINIMAP_PX_PER_U - ATLAS_PAGE_PX;

      const ground = tile.terrain.ground;
      for (let sy: u32 = 0; sy < ATLAS_PAGE_PX; sy++) {
        // Source raster row increases with world Y (same convention as get_ele's ty),
        // but atlas pixel rows increase downward (south) for a north-up image — flip.
        const py: f64 = py0 + (ATLAS_PAGE_PX - 1 - sy);
        const cy: u32 = sy * SAMPLE_STRIDE;

        for (let sx: u32 = 0; sx < ATLAS_PAGE_PX; sx++) {
          const cx: u32 = sx * SAMPLE_STRIDE;
          const off: u32 = cy * PAGE_CELL_SPAN + cx;

          // Same override order as the shader: later channels win over earlier ones
          // when both are present, regardless of their numeric type-id magnitude.
          let color: string = BASE_COLOR;
          for (const ch_key of GROUND_CH_KEYS) {
            const v: u8 = ground[ch_key][off];
            if (v === 0) continue; // AIR — channel not present at this cell
            const c = COLOR_TABLE[ch_key][v];
            if (c) color = c;
          }

          ctx.fillStyle = color;
          ctx.fillRect(px0 + sx, py, 1, 1);
        }
      }
    }

    set_atlas({ url: canvas.toDataURL(), origin });
    // tile_map/center_page_idx are the only two conditions that should trigger a rebake;
    // map_center itself changes far more often (every pan tick) and is handled by
    // minimap_pan_px() instead, not by redrawing the atlas.
    // deno-lint-ignore react-hooks/exhaustive-deps
  }, [tile_map, center_page_idx]);

  return atlas;
}

/** CSS translate (px) to pan an already-baked atlas so `map_center` sits at its visual center. */
export function minimap_pan_px(map_center: PVF3, origin: PVF3): { x: f64; y: f64 } {
  pvf3.sub($delta, map_center, origin);
  const dx: f64 = pvf3.to_real(pvf3.X($delta));
  const dy: f64 = pvf3.to_real(pvf3.Y($delta));
  return { x: -dx * MINIMAP_PX_PER_U, y: dy * MINIMAP_PX_PER_U };
}

/** Keeps the atlas legible at extreme zoom: the raster's own baked resolution limits
 * how far in it can go before looking blocky, and shrinking too far makes it useless. */
const MIN_MINIMAP_SCALE: f64 = 0.5;
const MAX_MINIMAP_SCALE: f64 = 2.0;

/** CSS scale for the minimap background so it zooms in/out along with the main camera —
 * `dist` further from DEFAULT_DIST (zoomed out) shows more area (scale < 1) and closer
 * (zoomed in) shows less (scale > 1), clamped so the fixed-resolution atlas stays legible. */
export function minimap_scale_for_dist(dist: f64): f64 {
  return clamp(DEFAULT_DIST / dist, MIN_MINIMAP_SCALE, MAX_MINIMAP_SCALE);
}

/** Diameter (px) of the baked atlas image — the consumer needs this to size/center it. */
export const MINIMAP_ATLAS_PX: u32 = ATLAS_PX;
