// @pxl/api/bake/terrain/pipeline/ground_baker.ts

/**
 * Rasterize normalized OSM features into the four ground channels of the current tile.
 *
 * Architectural notes:
 * - input features are interpreted relative to the current L3 page base
 * - raster rows follow south-to-north world ordering
 * - the raster contract is driven by the baked ground constants
 * - transient vertex/scratch storage uses arena-style reuse to reduce GC churn
 */

import type { f64, i32, u32, u8 } from "@xpute/core/kit/word.ts";
import type { F64Array, U8Array } from "@xpute/core/kit/array.ts";
import { Vector } from "@xpute/core/ds/arena.ts";

import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { type Ground, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { PAGE_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

import type { OSMFeature } from "@pxl/shared/bake/model/osm_feature.ts";

const PAGE_CELL_SPAN2: u32 = PAGE_CELL_SPAN * PAGE_CELL_SPAN;

/**
 * Module-static scanline intersection scratch.
 *
 * Invariant:
 * - render_polygon() is synchronous and non-reentrant
 * - do not await inside render_polygon()
 * - do not call render_polygon() concurrently
 *
 * Capacity:
 * - init_cap is only a hint
 * - tune it to exceed typical outer+hole intersection counts per scanline
 */
const XS_SCRATCH = new Vector(Float64Array, { init_cap: 1 << 10 });

/**
 * Sampling policy
 * - Ground rasterization uses cell-center sampling semantics.
 * - Keep center bias handling explicit and localized through PX_CENTER / SNAP_PX();
 *   do not sprinkle raw center-bias literals across the file.
 * - This contract must remain aligned with elevation sampling and dense unpack
 *   semantics at the page boundary.
 *
 * Metric relation:
 * - u_per_px  = TILE_U_PER_CELL
 * - px_per_u  = TILE_CELLS_PER_U
 */
const PX_CENTER: f64 = 0.5;
const EPS: f64 = 1e-9;

// pixel-center snap for raster coordinates
const SNAP_PX = (x: f64): i32 => Math.floor(x + PX_CENTER);

const CELL_IDX = (x: u32, y: u32): u32 => y * PAGE_CELL_SPAN + x;

const IS_SEA = (f: OSMFeature): boolean => f.extra.is_ocean;

export function bake_ground(elements: OSMFeature[], has_coast_row: boolean): Ground {
  const ground: Ground = {
    land: new Uint8Array(PAGE_CELL_SPAN2),
    water: new Uint8Array(PAGE_CELL_SPAN2),
    structure: new Uint8Array(PAGE_CELL_SPAN2),
    transport: new Uint8Array(PAGE_CELL_SPAN2),
  } as const;

  const sea_feats: OSMFeature[] = elements.filter(IS_SEA);
  const other_feats: OSMFeature[] = elements.filter((f) => !IS_SEA(f)).sort((a, b) => a.extra.rank - b.extra.rank);
  const has_sea: boolean = sea_feats.length > 0;

  // Inverse sea pass: initialize the land channel with the sea/land base first.
  if (has_sea) {
    ground.land.fill(cube_types.land.SEA);

    sea_feats.forEach((f) => {
      // only closed polygons carve land out of the sea base.
      if (f.shape.is_closed) {
        render_polygon(
          f.shape.points,
          cube_types.land.BARE,
          ground.land,
          f.shape.holes,
        );
      }
    });
  } else if (has_coast_row) {
    // no sea_feats reached here, but osm_adapter's from_coast_row() did see a
    // coastline row for this page (and dropped it — see its covers_page
    // comment): a single land polygon covers the page's full envelope, so
    // painting BARE directly is equivalent to carving it via render_polygon,
    // without paying for the scanline rasterization.
    ground.land.fill(cube_types.land.BARE);
  } else {
    // no coastline row touched this page's envelope at all — there's no
    // land anywhere nearby, so this is open ocean, not "no data yet".
    // Before this branch existed, this case fell through to the BARE
    // default above and every open-ocean page baked as flat dry land.
    ground.land.fill(cube_types.land.SEA);
  }

  // render categorized ground features.
  other_feats.forEach((feat) => {
    const { shape, extra, computed, cell_cfg } = feat;
    if (extra.is_tunnel) return;
    if (!cell_cfg) return;

    const { cell_type, ch_key } = cell_cfg;
    const target_ch = ground[ch_key];

    // water and land overlays must not paint over true SEA cells.
    const is_land_or_water: boolean = ch_key === GroundChannelKey.LAND || ch_key === GroundChannelKey.WATER;
    const mask_ch: U8Array | undefined = has_sea && is_land_or_water ? ground.land : undefined;
    const mask_val: u8 = cube_types.land.SEA;

    if (shape.is_closed) {
      render_polygon(
        shape.points,
        cell_type,
        target_ch,
        shape.holes,
        mask_ch,
        mask_val,
      );
    } else {
      render_linear_feat(
        shape.points,
        cell_type,
        target_ch,
        computed.width,
        mask_ch,
        mask_val,
      );
    }
  });

  return ground;
}

// ---- Rasterizer Core ----

/** Rasterize a polygon ring set with optional hole rings. */
function render_polygon(
  points: V3[],
  value: u8,
  target_ch: U8Array,
  holes?: V3[][],
  mask_ch?: U8Array,
  mask_val?: u8,
) {
  if (points.length < 3) return;

  const outer: V3[] = points;
  const inner_list: V3[][] = holes || [];

  // bounding box in dense pixel rows
  let min_y: f64 = Infinity;
  let max_y: f64 = -Infinity;
  for (let i: u32 = 0; i < outer.length; i++) {
    if (outer[i][1] < min_y) min_y = outer[i][1];
    if (outer[i][1] > max_y) max_y = outer[i][1];
  }

  const start_row: u32 = Math.max(0, Math.floor(min_y));
  const end_row: u32 = Math.min(PAGE_CELL_SPAN - 1, Math.ceil(max_y));

  // single even-odd scanline fill over outer + holes.
  for (let y: u32 = start_row; y <= end_row; y++) {
    using _ = XS_SCRATCH.scope;

    find_intersections(outer, y);
    for (let h: u32 = 0; h < inner_list.length; h++) find_intersections(inner_list[h], y);

    const valid_xs: F64Array = XS_SCRATCH.mem.subarray(0, XS_SCRATCH.len);
    valid_xs.sort();

    // preserve the old odd-intersection behavior without allocating/unshifting.
    const is_odd: boolean = valid_xs.length % 2 !== 0;

    let cur_idx: u32 = 0;
    if (is_odd) {
      const end_x: u32 = Math.min(PAGE_CELL_SPAN - 1, Math.floor(valid_xs[0]));
      for (let x: u32 = 0; x <= end_x; x++) {
        if (mask_ch && mask_val !== undefined) {
          if (mask_ch[CELL_IDX(x, y)] === mask_val) continue;
        }
        target_ch[CELL_IDX(x, y)] = value;
      }
      cur_idx = 1;
    }

    for (let i: u32 = cur_idx; i < valid_xs.length; i += 2) {
      if (i + 1 >= valid_xs.length) break;

      const start_x: u32 = Math.max(0, Math.ceil(valid_xs[i]));
      const end_x: u32 = Math.min(PAGE_CELL_SPAN - 1, Math.floor(valid_xs[i + 1]));

      for (let x: u32 = start_x; x <= end_x; x++) {
        if (mask_ch && mask_val !== undefined) {
          if (mask_ch[CELL_IDX(x, y)] === mask_val) continue;
        }
        target_ch[CELL_IDX(x, y)] = value;
      }
    }
  }
}

/** Rasterize an open polyline with a simple thickness kernel. */
function render_linear_feat(
  points: V3[],
  value: u8,
  target_ch: U8Array,
  width_cell: f64,
  mask_ch?: U8Array,
  mask_val?: u8,
) {
  if (!points || points.length < 2) return;

  const pixel_points: V3[] = points;

  const set_pixel = (px: i32, py: i32) => {
    if (px < 0 || px >= PAGE_CELL_SPAN || py < 0 || py >= PAGE_CELL_SPAN) return;

    if (mask_ch && mask_val !== undefined) {
      if (mask_ch[CELL_IDX(px as u32, py as u32)] === mask_val) return;
    }
    target_ch[CELL_IDX(px as u32, py as u32)] = value;
  };

  const draw_circle = (cx: i32, cy: i32, r: i32) => {
    const r2: i32 = r * r;
    for (let bx: i32 = -r; bx <= r; bx++) {
      for (let by: i32 = -r; by <= r; by++) {
        if (bx * bx + by * by <= r2) set_pixel(cx + bx, cy + by);
      }
    }
  };

  const thickness: i32 = Math.max(1, Math.round(width_cell * 0.5));

  for (let i: u32 = 0; i < pixel_points.length - 1; i++) {
    const p1: V3 = pixel_points[i];
    const p2: V3 = pixel_points[i + 1];

    // preserve the old tile-baker cap behavior at pixel centers.
    draw_circle(SNAP_PX(p1[0]), SNAP_PX(p1[1]), thickness);
    if (i === pixel_points.length - 2) {
      draw_circle(SNAP_PX(p2[0]), SNAP_PX(p2[1]), thickness);
    }

    const dx: f64 = p2[0] - p1[0];
    const dy: f64 = p2[1] - p1[1];
    // grid-cover sampling:
    // - use the max-axis delta for slope-independent coverage
    // - keep the legacy 2x stepping density used by the old tile baker
    const steps: u32 = Math.max(1, Math.ceil(Math.max(Math.abs(dx), Math.abs(dy)) * 2));

    for (let s: u32 = 0; s <= steps; s++) {
      const t: f64 = s / steps;
      const cx: i32 = SNAP_PX(p1[0] + dx * t);
      const cy: i32 = SNAP_PX(p1[1] + dy * t);

      for (let bx: i32 = -thickness; bx <= thickness; bx++) {
        for (let by: i32 = -thickness; by <= thickness; by++) {
          set_pixel(cx + bx, cy + by);
        }
      }
    }
  }
}

/** Collect scanline edge intersections in tile pixel space. */
function find_intersections(nodes: V3[], y: u32) {
  for (let i: u32 = 0, j: u32 = nodes.length - 1; i < nodes.length; j = i++) {
    const yi: f64 = nodes[i][1];
    const yj: f64 = nodes[j][1];
    const dy: f64 = yj - yi;
    if (Math.abs(dy) < EPS) continue;

    const ys: f64 = y + EPS;
    if ((yi <= ys && yj > ys) || (yj <= ys && yi > ys)) {
      const t: f64 = (ys - yi) / dy;
      XS_SCRATCH.push(nodes[i][0] + t * (nodes[j][0] - nodes[i][0]));
    }
  }
}
