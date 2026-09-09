// @pxl/web/render/layer/pipeline/chunk_mesher.test.ts

import { assert, assertEquals } from "@std/assert";

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constants.ts";

import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "./volume.ts";
import { type MeshRun, pack_column_runs } from "./voxel_scan.ts";
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

/** Every box in the bake, keyed by its own centre, with the face/shading
 * mask its vertices carry. Walks vertices rather than indexing by a stride,
 * because a chunk holding both terrain (open-bottom) and objects (six
 * faces) has no single stride to index by. */
function boxes_of(built: BakedChunkMesh): Map<string, number> {
  const out = new Map<string, number>();
  for (let v = 0; v < built.vertex_cnt; v++) {
    const o = v * 3;
    const c = [0, 1, 2].map((k) => built.position[o + k] - built.local_pos[o + k] * built.box_scale[o + k]);
    out.set(c.join(","), built.faces[v]);
  }
  return out;
}

// Bit values from chunk_mesher.ts's own mask layout.
const FACE_PX_BIT = 1 << 0;
const CREASE_PX_BIT = 1 << 18;
const CREASE_NX_BIT = 1 << 19;
const CAP_PZ_BIT = 1 << 22;
const RISES_PY_BIT = 1 << 8;

Deno.test("LOD1 - a merged box does not stamp one column's edge across its whole width", () => {
  const columns = empty_columns();
  // Two flat rows, four cells wide, all one run. Then a step up behind the
  // first column only, so exactly one cell of the far row has a concave
  // edge on its +Y side and the other three have open air.
  for (let cx: u32 = 5; cx <= 8; cx++) {
    columns[idx(cx, 0)] = [run(0, 0, 200, 100, 50)];
    columns[idx(cx, 1)] = [run(0, 0, 200, 100, 50)];
  }
  columns[idx(5, 2)] = [run(0, 1, 200, 100, 50)];

  const boxes = boxes_of(bake_chunk_mesh(pack_column_runs(columns), 1));
  // The cell under the step keeps its edge, and the three beside it do not
  // inherit it. merged_mask reads a box's +Y border from column 0 of its far
  // row, so a merge that spanned this row would have given all four the
  // first one's edge.
  assert((boxes.get("5.5,1.5,0.5")! & RISES_PY_BIT) !== 0);
  assertEquals(boxes.get("7.5,1.5,0.5")! & RISES_PY_BIT, 0);
});

Deno.test("LOD1 - a user cube flush against terrain draws no seam between them", () => {
  const columns = empty_columns();
  // Same height, side by side, one LAND and one OBJECT. Nothing about that
  // join is a silhouette, so neither side may claim a convex crease — a
  // placed cube set into flat ground has to read as flat ground.
  columns[idx(5, 5)] = [run(0, 0, 200, 100, 50, MaterialClass.LAND)];
  columns[idx(6, 5)] = [run(0, 0, 50, 100, 200, MaterialClass.OBJECT)];

  const boxes = boxes_of(bake_chunk_mesh(pack_column_runs(columns), 1));
  const land = boxes.get("5.5,5.5,0.5")!;
  const cube = boxes.get("6.5,5.5,0.5")!;
  assertEquals(land & CREASE_PX_BIT, 0);
  assertEquals(cube & CREASE_NX_BIT, 0);
  // The face itself still survives: an object stays exempt from culling so
  // it can never open a hole under or beside itself.
  assert((land & FACE_PX_BIT) !== 0);
});

Deno.test("LOD1 - a wall capped by a user cube is not shaded as a silhouette", () => {
  const columns = empty_columns();
  // Terrain with a placed cube sitting on it. The terrain's +Z face is kept
  // (objects never cull), but its side walls are capped all the same, so
  // their top edges are not open air.
  columns[idx(5, 5)] = [run(0, 0, 200, 100, 50, MaterialClass.LAND), run(1, 1, 50, 100, 200, MaterialClass.OBJECT)];

  const boxes = boxes_of(bake_chunk_mesh(pack_column_runs(columns), 1));
  assert((boxes.get("5.5,5.5,0.5")! & CAP_PZ_BIT) !== 0);
  // Nothing above the cube itself, so it stays uncapped.
  assertEquals(boxes.get("5.5,5.5,1.5")! & CAP_PZ_BIT, 0);
});

Deno.test("coarse mesher - a fully-covered cell doesn't shrink and blends its columns' colors", () => {
  const columns = empty_columns();
  // lod_step=2: all 4 sub-columns of the one coarse cell, same color/z.
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) {
    columns[idx(cx, cy)] = [run(4, 5, 255, 0, 0)];
  }

  const built = bake_chunk_mesh(pack_column_runs(columns), 2);
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

  const built = bake_chunk_mesh(pack_column_runs(columns), 2);
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

  const built = bake_chunk_mesh(pack_column_runs(columns), 2);
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
    bake_chunk_mesh_coarse(pack_column_runs(empty_columns()), 1);
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
  const built = bake_chunk_mesh(pack_column_runs(columns), 1);
  // Five runs survive whole-run culling (4 neighbours + the centre's own
  // STRUCTURE top; its buried LAND run is the one culled), but they no
  // longer each emit five faces — run_face_mask drops the ones nothing can
  // see. The STRUCTURE run sits a level above every neighbour, so all four
  // of its sides are exposed: 5 faces. Each neighbour is flush against the
  // centre column on exactly one side: 4 faces each.
  assertEquals(built.vertex_cnt, (5 + 4 * 4) * 4);
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
  const built = bake_chunk_mesh(pack_column_runs(columns), 1);
  // Nothing is whole-run culled, and the object's presence must not cost the
  // terrain under it its top face — that is this test's original point and
  // face culling has to honour it too, which is why the coverage test it
  // uses skips OBJECT runs.
  //
  // The centre's terrain run keeps only that top face: all four neighbours
  // are flush with it. The object keeps five — every side and its top, but
  // not its bottom, which the terrain directly beneath really does hide.
  // Each neighbour keeps four, flush against the centre on one side.
  assertEquals(built.vertex_cnt, (1 + 5 + 4 * 4) * 4);
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

  const built = bake_chunk_mesh(pack_column_runs(columns), 1);
  // Nothing is whole-run culled, and the edge face is kept for the same
  // reason the run is: the neighbouring chunk's data is not available here,
  // so its -X side stays even though the other three are flush.
  //
  // Edge terrain run: top + the -X edge = 2. Object on it: five, as above.
  // The three neighbours: four each, flush against the edge column on one
  // side — including the two that are themselves on the x=0 edge and so
  // keep their own -X faces.
  assertEquals(built.vertex_cnt, (2 + 5 + 3 * 4) * 4);
});

Deno.test("coarse_instance_stats tracks low-fill instances the always-shrink policy would have dropped under a threshold rule", () => {
  const columns = empty_columns();
  columns[idx(2, 2)] = [run(4, 4, 200, 100, 50)]; // fill = 1/8, well under a 25% threshold

  const total_before = coarse_instance_stats.total;
  const low_fill_before = coarse_instance_stats.low_fill;
  bake_chunk_mesh(pack_column_runs(columns), 2);
  assertEquals(coarse_instance_stats.total, total_before + 1);
  assertEquals(coarse_instance_stats.low_fill, low_fill_before + 1);
});

Deno.test("bake_chunk_mesh - a box's baked position matches its own real-unit center", () => {
  const columns = empty_columns();
  columns[idx(1, 2)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];

  const built = bake_chunk_mesh(pack_column_runs(columns), 1);
  assertEquals(box_cnt(built, 5), 1);
  assertEquals(box_center_of(built, 0, 5), [1.5, 2.5, 0.5]);
});

/** The case face culling exists for, and the one the whole-run test above
 * cannot catch: a flat plateau culls nothing at run granularity — every
 * column's run is topmost, so none is buried — while almost every face
 * between them is hidden. */
Deno.test("LOD1 - a flat plateau emits its top faces and its rim, not five faces per cell", () => {
  const columns = empty_columns();
  const lo: u32 = 4, hi: u32 = 8; // a 5x5 block of columns, clear of the chunk edge
  for (let cx = lo; cx <= hi; cx++) {
    for (let cy = lo; cy <= hi; cy++) columns[idx(cx, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  }

  const culled_before = occlusion_stats.culled;
  const built = bake_chunk_mesh(pack_column_runs(columns), 1);

  // The whole plateau is one box. Every cell holds the same run — same z
  // span, same material — and no interior border carries a crease, so the
  // greedy merge takes all 25 into a single 5x5x1 box with a top and four
  // rim faces. Five faces where an unmerged, unculled bake emitted 125.
  assertEquals(built.vertex_cnt, 5 * 4);
  // ...and not one run was occlusion-culled, which is the point: the two
  // tests are looking at different things.
  assertEquals(occlusion_stats.culled, culled_before);
});
