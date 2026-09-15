// @pxl/web/reference/chunk_mesher.test.ts

import { assert, assertEquals } from "@std/assert";

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";

import { MaterialClass, NO_LAYER, SUB_CELL_NONE } from "@/reference/mesh/voxel.ts";
import { type MeshRun, pack_column_runs } from "@/reference/voxel_scan.ts";
import { abort_bake, bake_chunk_mesh, bake_chunk_mesh_coarse, bake_open, BakeContext, type BakedChunkMesh, begin_bake, finish_bake, step_bake } from "@/reference/chunk_mesher.ts";
import { BuddyAllocator } from "@xpute/core/ds/buddy.ts";
import { buddy_alloc } from "@xpute/core/ds/typed_alloc.ts";
import { release_chunk_mesh } from "@/reference/core_pool.ts";
import { CORE_BAKE } from "@/reference/chunk_bake.ts";

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

  const boxes = boxes_of(bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1));
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

  const boxes = boxes_of(bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1));
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

  const boxes = boxes_of(bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1));
  assert((boxes.get("5.5,5.5,0.5")! & CAP_PZ_BIT) !== 0);
  // Nothing above the cube itself, so it stays uncapped.
  assertEquals(boxes.get("5.5,5.5,1.5")! & CAP_PZ_BIT, 0);
});

Deno.test("LOD1 - a carved-and-refilled cell draws no outline in the flat ground around it", () => {
  const columns = empty_columns();
  // Flat plateau, one run per column, then one cell carved and refilled
  // with another material: that column becomes terrain below plus an
  // object on top, at the same height as everything around it. Nothing
  // about that join is a silhouette, so no neighbour may claim a crease —
  // and a neighbour's side is covered by those two runs together even
  // though no single one spans it.
  for (let cx: u32 = 3; cx <= 9; cx++) {
    for (let cy: u32 = 3; cy <= 9; cy++) columns[idx(cx, cy)] = [run(0, 2, 200, 100, 50)];
  }
  columns[idx(6, 6)] = [
    run(0, 1, 200, 100, 50),
    run(2, 2, 50, 100, 200, MaterialClass.OBJECT),
  ];

  const boxes = boxes_of(bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1));
  // Merged boxes, so these are the real centres — asserted present first,
  // since `undefined & bit` is 0 and would pass every check below silently.
  for (const [key, inward] of [["4.5,6.5,1.5", CREASE_PX_BIT], ["8.5,6.5,1.5", CREASE_NX_BIT]] as const) {
    assert(boxes.has(key), `expected a box at ${key}`);
    assertEquals(boxes.get(key)! & inward, 0);
  }
  // The refilled cube is flush on every side too.
  assert(boxes.has("6.5,6.5,2.5"));
  assertEquals(boxes.get("6.5,6.5,2.5")! & (CREASE_PX_BIT | CREASE_NX_BIT), 0);
});

Deno.test("coarse mesher - a fully-covered cell doesn't shrink and blends its columns' colors", () => {
  const columns = empty_columns();
  // lod_step=2: all 4 sub-columns of the one coarse cell, same color/z.
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) {
    columns[idx(cx, cy)] = [run(4, 5, 255, 0, 0)];
  }

  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 2);
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

  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 2);
  assertEquals(box_cnt(built, 5), 1); // coarse output is always one merged, open-bottom box
  assert(built.rgb[0] > 0); // red contributed
  assert(built.rgb[2] > 0); // blue contributed too — genuinely blended, not one or the other
});

Deno.test("coarse mesher - a sparse cell shrinks isotropically about its cell centre", () => {
  const sparse = empty_columns();
  // 1 of this lod_step=4 cell's 64 fine voxels: fill 1/64, so the box is
  // cbrt(1/64) = a quarter of the cell on every axis, and it sits at the
  // cell's centre rather than at the voxel's own place. Density is what the
  // size means here; position within the cell is below this tier.
  sparse[idx(1, 2)] = [run(4, 4, 200, 100, 50)];

  const lone = bake_chunk_mesh(CORE_BAKE, pack_column_runs(sparse), 4);
  assertEquals(box_cnt(lone, 5), 1); // open-bottom, like every coarse box
  assertEquals(box_scale_of(lone, 0, 5), [1, 1, 1]);
  assertEquals(box_center_of(lone, 0, 5), [2, 2, 6]);

  const filled = empty_columns();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) filled[idx(cx, cy)] = [run(4, 5, 200, 100, 50)];
  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(filled), 2);
  assertEquals(box_cnt(built, 5), 1);
  // Full cell: no shrink, and it stays a plain full-cell run that can still
  // merge and cull against its neighbours.
  assertEquals(box_scale_of(built, 0, 5), [2, 2, 2]);
});

Deno.test("coarse mesher - finest LOD still requires per-run meshing", () => {
  let threw = false;
  try {
    bake_chunk_mesh_coarse(CORE_BAKE, pack_column_runs(empty_columns()), 1);
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

  const culled_before = CORE_BAKE.occlusion.culled;
  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1);
  // Five runs survive whole-run culling (4 neighbours + the centre's own
  // STRUCTURE top; its buried LAND run is the one culled), but they no
  // longer each emit five faces — run_face_mask drops the ones nothing can
  // see. The STRUCTURE run sits a level above every neighbour, so all four
  // of its sides are exposed: 5 faces. Each neighbour is flush against the
  // centre column on exactly one side: 4 faces each.
  assertEquals(built.vertex_cnt, (5 + 4 * 4) * 4);
  assertEquals(CORE_BAKE.occlusion.culled, culled_before + 1);
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

  const culled_before = CORE_BAKE.occlusion.culled;
  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1);
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
  assertEquals(CORE_BAKE.occlusion.culled, culled_before);
});

Deno.test("LOD1 - a chunk-edge column is never occlusion-culled even if it looks fully surrounded on its other sides", () => {
  const columns = empty_columns();
  const cx: u32 = 0, cy: u32 = 5; // cx=0 is a chunk edge — neighbor chunk's data is unknown here.

  columns[idx(cx, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND), run(1, 1, 200, 100, 50, MaterialClass.OBJECT)];
  columns[idx(cx + 1, cy)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy - 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  columns[idx(cx, cy + 1)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];
  // (cx-1, cy) doesn't exist — this column is at the chunk's own edge.

  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1);
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

Deno.test("coarse_instance_stats counts cells too diffuse for any single box to represent", () => {
  const columns = empty_columns();
  // Two opposite corners of a lod_step=4 cell, one cell tall: the fitted box
  // is the full 4x4x1 and holds two voxels, an eighth full. Unlike a lone
  // block, no box of any size describes this — so it is dropped, and that is
  // what the counter is for.
  columns[idx(0, 0)] = [run(4, 4, 200, 100, 50)];
  columns[idx(3, 3)] = [run(4, 4, 200, 100, 50)];

  const total_before = CORE_BAKE.coarse_instances.total;
  const low_fill_before = CORE_BAKE.coarse_instances.low_fill;
  bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 4);
  assertEquals(CORE_BAKE.coarse_instances.total, total_before + 1);
  assertEquals(CORE_BAKE.coarse_instances.low_fill, low_fill_before + 1);
});

Deno.test("bake_chunk_mesh - a box's baked position matches its own real-unit center", () => {
  const columns = empty_columns();
  columns[idx(1, 2)] = [run(0, 0, 10, 20, 30, MaterialClass.LAND)];

  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1);
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

  const culled_before = CORE_BAKE.occlusion.culled;
  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1);

  // The whole plateau is one box. Every cell holds the same run — same z
  // span, same material — and no interior border carries a crease, so the
  // greedy merge takes all 25 into a single 5x5x1 box with a top and four
  // rim faces. Five faces where an unmerged, unculled bake emitted 125.
  assertEquals(built.vertex_cnt, 5 * 4);
  // ...and not one run was occlusion-culled, which is the point: the two
  // tests are looking at different things.
  assertEquals(CORE_BAKE.occlusion.culled, culled_before);
});

// ---- Edge-mask symmetry ----
//
// The lateral half of the face mask is four parallel groups, each ordered
// +X, -X, +Y, -Y, plus four diagonals named by their X then Y sign. Nothing
// in the rule that fills them may prefer an axis or a direction, so
// reflecting or transposing a scene has to reflect or transpose its masks
// and change nothing else. Asserted as an identity over a whole generated
// scene rather than sampled at a few cells: a rule that leans one way
// usually does it in one configuration, which is what a stray single line
// on flat ground looks like from the outside.

const LATERAL_BASES = [FACE_PX_BIT, 1 << 6, 1 << 10, CREASE_PX_BIT] as const;
const DIAG_PP_BIT = 1 << 14, DIAG_PN_BIT = 1 << 15, DIAG_NP_BIT = 1 << 16, DIAG_NN_BIT = 1 << 17;

/** Rewrites a mask's lateral bits under a permutation of (+X,-X,+Y,-Y),
 * given as destination slots for slots 0..3, plus one for the diagonals. */
function remap_mask(mask: number, lateral: readonly [number, number, number, number], diag: readonly [number, number, number, number]): number {
  let out = mask;
  for (const base of LATERAL_BASES) {
    out &= ~(base * 15);
    for (let slot = 0; slot < 4; slot++) if ((mask & (base << slot)) !== 0) out |= base << lateral[slot];
  }
  const diag_bits = [DIAG_PP_BIT, DIAG_PN_BIT, DIAG_NP_BIT, DIAG_NN_BIT];
  out &= ~(DIAG_PP_BIT | DIAG_PN_BIT | DIAG_NP_BIT | DIAG_NN_BIT);
  for (let slot = 0; slot < 4; slot++) if ((mask & diag_bits[slot]) !== 0) out |= diag_bits[diag[slot]];
  return out;
}

/** Deterministic mixed scene: varied heights, terrain and object columns
 * interleaved, some columns split into two runs the way a carve-and-refill
 * leaves them. Confined to 8..24 on both axes so mirroring about 32 and
 * transposing both land back inside it, keeping chunk-edge exemptions out
 * of the comparison. */
function mixed_scene(place: (cx: u32, cy: u32) => [u32, u32]): MeshRun[][] {
  const columns = empty_columns();
  for (let x: u32 = 8; x <= 24; x++) {
    for (let y: u32 = 8; y <= 24; y++) {
      const h = (x * 7 + y * 13) % 4;
      const kind = (x * 3 + y * 5) % 4;
      const [px, py] = place(x, y);
      if (kind === 0) columns[idx(px, py)] = [run(0, h, 200, 100, 50)];
      else if (kind === 1) columns[idx(px, py)] = [run(0, h, 60, 160, 90, MaterialClass.OBJECT)];
      else if (kind === 2) {
        columns[idx(px, py)] = h === 0 ? [run(0, 0, 60, 160, 90, MaterialClass.OBJECT)] : [run(0, h - 1, 200, 100, 50), run(h, h, 60, 160, 90, MaterialClass.OBJECT)];
      } else columns[idx(px, py)] = [run(0, h, 120, 120, 200)];
    }
  }
  return columns;
}

function masks_by_centre(columns: MeshRun[][]): Map<string, number> {
  return boxes_of(bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1));
}

Deno.test("edge mask - mirroring X mirrors the mask and changes nothing else", () => {
  const direct = masks_by_centre(mixed_scene((x, y) => [x, y]));
  const mirrored = masks_by_centre(mixed_scene((x, y) => [32 - x, y]));
  assert(direct.size > 100, "scene should produce a real number of boxes");
  assertEquals(direct.size, mirrored.size);
  for (const [key, mask] of direct) {
    const [cx, cy, cz] = key.split(",").map(Number);
    const image = `${33 - cx},${cy},${cz}`;
    assert(mirrored.has(image), `mirrored box missing at ${image}`);
    assertEquals(mirrored.get(image), remap_mask(mask, [1, 0, 2, 3], [2, 3, 0, 1]), `at ${key}`);
  }
});

Deno.test("edge mask - transposing X and Y transposes the mask and changes nothing else", () => {
  const direct = masks_by_centre(mixed_scene((x, y) => [x, y]));
  const transposed = masks_by_centre(mixed_scene((x, y) => [y, x]));
  assertEquals(direct.size, transposed.size);
  for (const [key, mask] of direct) {
    const [cx, cy, cz] = key.split(",").map(Number);
    const image = `${cy},${cx},${cz}`;
    assert(transposed.has(image), `transposed box missing at ${image}`);
    assertEquals(transposed.get(image), remap_mask(mask, [2, 3, 0, 1], [0, 2, 1, 3]), `at ${key}`);
  }
});

/** Centre, per-axis extent and mask for every baked box — the rim test
 * needs the extent, since a merged box's centre says nothing about whether
 * it reaches the edge of the scene. */
function boxes_with_extent(columns: MeshRun[][]): { c: number[]; s: number[]; mask: number }[] {
  const built = bake_chunk_mesh(CORE_BAKE, pack_column_runs(columns), 1);
  const out = new Map<string, { c: number[]; s: number[]; mask: number }>();
  for (let v = 0; v < built.vertex_cnt; v++) {
    const o = v * 3;
    const c = [0, 1, 2].map((i) => built.position[o + i] - built.local_pos[o + i] * built.box_scale[o + i]);
    const s = [0, 1, 2].map((i) => built.box_scale[o + i]);
    out.set(c.join(","), { c, s, mask: built.faces[v] });
  }
  return [...out.values()];
}

Deno.test("edge mask - a flat surface carries no crease except at its rim", () => {
  // Every column the same height, materials mixed freely, some split into
  // two runs the way a carve-and-refill leaves them. A continuous surface
  // has no silhouette anywhere inside it, so the only lateral creases
  // allowed belong to boxes that actually reach the outer ring.
  const columns = empty_columns();
  const LO: u32 = 8, HI: u32 = 24;
  let kinds = 0;
  for (let x = LO; x <= HI; x++) {
    for (let y = LO; y <= HI; y++) {
      const kind = (x * 5 + y * 7) % 3;
      kinds |= 1 << kind;
      columns[idx(x, y)] = kind === 0
        ? [run(0, 2, 200, 100, 50)]
        : kind === 1
        ? [run(0, 2, 60, 160, 90, MaterialClass.OBJECT)]
        : [run(0, 1, 200, 100, 50), run(2, 2, 60, 160, 90, MaterialClass.OBJECT)];
    }
  }
  assertEquals(kinds, 0b111, "the scene must actually mix all three column shapes");

  const any_crease = CREASE_PX_BIT | CREASE_NX_BIT | (1 << 20) | (1 << 21);
  const boxes = boxes_with_extent(columns);
  assert(boxes.length > 0);
  for (const { c, s, mask } of boxes) {
    if ((mask & any_crease) === 0) continue;
    const reaches_rim = c[0] - s[0] / 2 <= LO || c[0] + s[0] / 2 >= HI + 1 ||
      c[1] - s[1] / 2 <= LO || c[1] + s[1] / 2 >= HI + 1;
    assert(reaches_rim, `crease inside a flat surface at ${c.join(",")} (extent ${s.join("x")}, mask ${mask})`);
  }
});

/** Terrain with a different height in almost every column, and a floating
 * cube every few cells: enough variety that both the merge and the coarse
 * accumulation have real decisions at every step boundary. */
function busy_columns(): MeshRun[][] {
  const columns = empty_columns();
  for (let cy = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const h = (cx * 7 + cy * 13) % 5;
      columns[idx(cx, cy)].push(run(-3, h, 40 + cx, 80, 40 + cy));
      if ((cx + cy) % 11 === 0) columns[idx(cx, cy)].push(run(h + 2, h + 2, 200, 10, 10));
    }
  }
  return columns;
}

function same_mesh(a: BakedChunkMesh, b: BakedChunkMesh): void {
  assertEquals(a.vertex_cnt, b.vertex_cnt);
  assertEquals(Array.from(a.position), Array.from(b.position));
  assertEquals(Array.from(a.rgb), Array.from(b.rgb));
  assertEquals(Array.from(a.faces), Array.from(b.faces));
  assertEquals(Array.from(a.index), Array.from(b.index));
}

Deno.test("stepped bake - stepping bakes exactly what the whole bake bakes, at every tier", () => {
  for (const step of [1, 2, 4, 8] as const) {
    const whole = bake_chunk_mesh(CORE_BAKE, pack_column_runs(busy_columns()), step);

    const cursor = begin_bake(CORE_BAKE, pack_column_runs(busy_columns()), step);
    assert(cursor !== null);
    let steps = 0;
    while (!step_bake(cursor)) steps++;
    const stepped = finish_bake(cursor);

    assert(steps > 2, `tier ${step} took only ${steps} steps`);
    same_mesh(stepped, whole);
    release_chunk_mesh(whole);
    release_chunk_mesh(stepped);
  }
});

Deno.test("stepped bake - only one may be open, and an abort leaves the scratch as it found it", () => {
  const set = pack_column_runs(busy_columns());
  const before_fine = bake_chunk_mesh(CORE_BAKE, set, 1);
  const before_coarse = bake_chunk_mesh(CORE_BAKE, set, 4);

  // Abort a fine bake deep enough that it holds a builder block.
  const fine = begin_bake(CORE_BAKE, set, 1);
  assert(fine !== null);
  assertEquals(begin_bake(CORE_BAKE, set, 2), null);
  assertEquals(bake_open(CORE_BAKE), true);
  for (let i = 0; i < 17; i++) step_bake(fine);
  abort_bake(fine);
  assertEquals(bake_open(CORE_BAKE), false);

  // And a coarse one part-way through accumulating.
  const coarse = begin_bake(CORE_BAKE, set, 4);
  assert(coarse !== null);
  step_bake(coarse);
  abort_bake(coarse);
  assertEquals(bake_open(CORE_BAKE), false);

  // Nothing the aborted bakes left behind changes what the next ones build.
  const after_fine = bake_chunk_mesh(CORE_BAKE, set, 1);
  const after_coarse = bake_chunk_mesh(CORE_BAKE, set, 4);
  same_mesh(after_fine, before_fine);
  same_mesh(after_coarse, before_coarse);
  for (const m of [before_fine, before_coarse, after_fine, after_coarse]) release_chunk_mesh(m);
});

Deno.test("stepped bake - a pool that refuses the mesh's block starves the bake, which keeps its place and finishes once there is room", () => {
  const backing = new ArrayBuffer(1 << 20);
  const pool = new BuddyAllocator({ min_block_bytes: 1 << 12, region_bytes: 1 << 20, backing, no_spill: true });
  // Scratch from a pool of its own, so the section below holds meshes only.
  const ctx = new BakeContext(pool, buddy_alloc(new BuddyAllocator()));
  const set = pack_column_runs(busy_columns());
  // Take the whole section first, so the mesh has nowhere to go.
  const hog = pool.acquire(1 << 20);
  const cursor = begin_bake(ctx, set, 8)!;
  let steps = 0;
  while (!step_bake(cursor) && !cursor.starved) steps++;
  assert(cursor.starved, "the bake reports the refusal rather than throwing");
  assert(!cursor.done);
  const kept_before = ctx.occlusion.faces_kept;
  assert(!step_bake(cursor) && cursor.starved, "still starved on the next step");
  assertEquals(ctx.occlusion.faces_kept, kept_before, "the counting pass does not run again");
  pool.release(hog);
  while (!step_bake(cursor)) { /* room now */ }
  assert(!cursor.starved);
  const mesh = finish_bake(cursor);
  const direct = bake_chunk_mesh(CORE_BAKE, set, 8);
  assertEquals(mesh.vertex_cnt, direct.vertex_cnt);
  pool.release(mesh.block);
  release_chunk_mesh(direct);
});
