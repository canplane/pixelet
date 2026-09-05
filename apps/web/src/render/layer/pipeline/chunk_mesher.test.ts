// @pxl/web/render/layer/pipeline/chunk_mesher.test.ts

import { assert, assertEquals } from "@std/assert";

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "./volume.ts";
import type { MeshRun } from "./voxel_scan.ts";
import { bake_chunk_mesh, bake_chunk_mesh_coarse, type BakedChunkMesh, coarse_instance_stats, occlusion_stats } from "./chunk_mesher.ts";

function empty_columns(): MeshRun[][] {
  return Array.from({ length: CHUNK_CELL_SPAN * CHUNK_CELL_SPAN }, () => []);
}

function idx(cx: u32, cy: u32): u32 {
  return cy * CHUNK_CELL_SPAN + cx;
}

function run(
  z0: i32,
  z1: i32,
  r: u32,
  g: u32,
  b: u32,
  material_class: MaterialClass = MaterialClass.LAND,
): MeshRun {
  return { z0, z1, r, g, b, layer_idx: NO_LAYER, material_class, sub_sz: SUB_CELL_NONE, sub_ox: 0, sub_oy: 0, sub_oz: 0 };
}

/** One baked box's own face count from its vertex count — 4 verts/face, and
 * every emitted box either drops the bottom face (5) or keeps it (6). */
function box_cnt(built: BakedChunkMesh, faces_per_box: 5 | 6): number {
  return built.vertex_cnt / (faces_per_box * 4);
}

/** A baked box's own real-unit size — reads straight off any one of its
 * baked vertices' own a_box_scale copy (identical across the whole box). */
function box_scale_of(built: BakedChunkMesh, box_index: number, faces_per_box: 5 | 6): [number, number, number] {
  const vi = box_index * faces_per_box * 4;
  return [built.box_scale[vi * 3], built.box_scale[vi * 3 + 1], built.box_scale[vi * 3 + 2]];
}

/** A baked box's own real-unit center, exact from any single one of its own
 * vertices: emit_box bakes `position = center + local_pos * box_scale`, so
 * `center = position - local_pos * box_scale` recovers it directly —
 * reading its very first vertex is enough, not an average across a face
 * (whose 4 corners average to that FACE's own center, offset from the
 * box's true center by 0.5*normal*scale, not to the box center itself). */
function box_center_of(built: BakedChunkMesh, box_index: number, faces_per_box: 5 | 6): [number, number, number] {
  const vi = box_index * faces_per_box * 4 * 3;
  return [0, 1, 2].map((k) => built.position[vi + k] - built.local_pos[vi + k] * built.box_scale[vi + k]) as [number, number, number];
}

Deno.test("coarse mesher - a fully-covered cell doesn't shrink and blends its columns' colors", () => {
  const columns = empty_columns();
  // lod_step=2: all 4 sub-columns of the one coarse cell, same color/z.
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) {
    columns[idx(cx, cy)] = [run(4, 5, 255, 0, 0)];
  }

  const built = bake_chunk_mesh(columns, 2);
  assertEquals(box_cnt(built, 5), 1); // coarse boxes are always open-bottom
  assertEquals(box_scale_of(built, 0, 5)[2], 2); // span(2) * cbrt(fill=1) = 2, no shrink
  assert(built.rgb[0] > 0); // red channel present
  assertEquals(built.rgb[2], 0); // no blue contributed anywhere
});

Deno.test("coarse mesher - terrain and object columns in the same cell blend into one color", () => {
  const columns = empty_columns();
  // 2 red LAND columns + 2 blue OBJECT columns, same Z range: material
  // class is intentionally ignored by accumulate_run — this is the
  // mip/cone-tracing-style filter, not a per-class channel split.
  columns[idx(0, 0)] = [run(4, 5, 255, 0, 0, MaterialClass.LAND)];
  columns[idx(1, 0)] = [run(4, 5, 255, 0, 0, MaterialClass.LAND)];
  columns[idx(0, 1)] = [run(4, 5, 0, 0, 255, MaterialClass.OBJECT)];
  columns[idx(1, 1)] = [run(4, 5, 0, 0, 255, MaterialClass.OBJECT)];

  const built = bake_chunk_mesh(columns, 2);
  assertEquals(box_cnt(built, 5), 1); // coarse output is always one merged, open-bottom box
  assert(built.rgb[0] > 0); // red contributed
  assert(built.rgb[2] > 0); // blue contributed too — genuinely blended, not one or the other
});

Deno.test("coarse mesher - a partially-covered cell shrinks isotropically instead of vanishing", () => {
  const columns = empty_columns();
  // Only 1 of 4 sub-columns filled, one cell tall: fill = 1/8, box shrinks
  // to cbrt(1/8) = 0.5 of the full span on every axis — no occupancy
  // threshold drops it.
  columns[idx(0, 0)] = [run(4, 4, 200, 100, 50)];

  const built = bake_chunk_mesh(columns, 2);
  assertEquals(box_cnt(built, 5), 1);
  const expected_scale = 2 * Math.cbrt(1 / 8);
  const [sx, sy, sz] = box_scale_of(built, 0, 5);
  assertEquals(sx, expected_scale);
  assertEquals(sy, expected_scale);
  assertEquals(sz, expected_scale); // Z — same factor, isotropic
});

Deno.test("coarse mesher - finest LOD still requires per-run meshing", () => {
  let threw = false;
  try {
    bake_chunk_mesh_coarse(empty_columns(), 1);
  } catch {
    threw = true;
  }
  assert(threw);
});

Deno.test("LOD1 - a terrain run fully boxed in by solid terrain neighbors and covered by more terrain above is occlusion-culled", () => {
  const columns = empty_columns();
  const cx: u32 = 5, cy: u32 = 5; // well clear of chunk-edge columns (never culled)

  // Center column: terrain at the bottom, more terrain stacked directly
  // above it — a genuine continuous mass, not a standalone object, so the
  // bottom run is no longer this column's topmost run in the sense that
  // matters for culling.
  columns[idx(cx, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND), run(1, 1, 40, 20, 10, MaterialClass.STRUCTURE)];
  // All 4 horizontal neighbors are solid across the buried run's exact
  // z0..z1 span, so no side face of it is ever exposed either.
  columns[idx(cx - 1, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx + 1, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy - 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy + 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];

  const culled_before = occlusion_stats.culled;
  const built = bake_chunk_mesh(columns, 1);
  // 4 neighbor terrain runs (each topmost in its own column, never culled)
  // + the center column's own top (STRUCTURE) run + 0 for its buried
  // bottom (LAND) run — all non-OBJECT, so all open-bottom (5 faces).
  assertEquals(box_cnt(built, 5), 5);
  assertEquals(occlusion_stats.culled, culled_before + 1);
});

Deno.test("LOD1 - an object placed on terrain does not occlusion-cull the terrain beneath it", () => {
  const columns = empty_columns();
  const cx: u32 = 5, cy: u32 = 5; // well clear of chunk-edge columns (never culled)

  // Same shape as the terrain-over-terrain case above, but the thing on
  // top is a standalone OBJECT instead of more terrain — a real placed
  // object never actually seals its cell's top face from this game's
  // oblique (not straight-down) camera the way a continuous overlying
  // layer does, so the terrain underneath must stay visible (bug found
  // 2026-08-16: it was being culled outright, showing as a hole under
  // every placed object).
  columns[idx(cx, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND), run(1, 1, 200, 100, 50, MaterialClass.OBJECT)];
  columns[idx(cx - 1, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx + 1, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy - 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy + 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];

  const culled_before = occlusion_stats.culled;
  const built = bake_chunk_mesh(columns, 1);
  // 4 neighbor terrain runs + the center's own terrain run (5, open-bottom,
  // none culled) + the object on top (1, full box — OBJECT keeps its bottom
  // face). Total vertices = 5*5*4 + 1*6*4 = 124, not evenly divisible by
  // one single faces_per_box, so assert the two classes directly instead of
  // reusing box_cnt's single-shape assumption.
  assertEquals(built.vertex_cnt, 5 * 5 * 4 + 1 * 6 * 4);
  assertEquals(occlusion_stats.culled, culled_before);
});

Deno.test("LOD1 - a chunk-edge column is never occlusion-culled even if it looks fully surrounded on its other sides", () => {
  const columns = empty_columns();
  const cx: u32 = 0, cy: u32 = 5; // cx=0 is a chunk edge — neighbor chunk's data is unknown here.

  columns[idx(cx, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND), run(1, 1, 200, 100, 50, MaterialClass.OBJECT)];
  columns[idx(cx + 1, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy - 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy + 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  // (cx-1, cy) doesn't exist — this column is at the chunk's own edge.

  const built = bake_chunk_mesh(columns, 1);
  // The edge column's own terrain run is never culled: 1 (edge) + 3
  // (neighbors, each topmost in their own column) = 4 open-bottom terrain
  // boxes, plus 1 full-box object.
  assertEquals(built.vertex_cnt, 4 * 5 * 4 + 1 * 6 * 4);
});

Deno.test("coarse_instance_stats tracks low-fill instances the always-shrink policy would have dropped under a threshold rule", () => {
  const columns = empty_columns();
  columns[idx(2, 2)] = [run(4, 4, 200, 100, 50)]; // fill = 1/8, well under a 25% threshold

  const total_before = coarse_instance_stats.total;
  const low_fill_before = coarse_instance_stats.low_fill;
  bake_chunk_mesh(columns, 2);
  assertEquals(coarse_instance_stats.total, total_before + 1);
  assertEquals(coarse_instance_stats.low_fill, low_fill_before + 1);
});

Deno.test("bake_chunk_mesh - a box's baked position matches its own real-unit center", () => {
  const columns = empty_columns();
  columns[idx(1, 2)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];

  const built = bake_chunk_mesh(columns, 1);
  assertEquals(box_cnt(built, 5), 1);
  assertEquals(box_center_of(built, 0, 5), [1.5, 2.5, 0.5]);
});
