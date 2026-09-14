// @pxl/web/reference/chunk_bake.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";

import { mat4, mat4_frustum_planes, mat4_look_at, mat4_mul, mat4_perspective } from "@xpute/core/kit/mat4.ts";

import { alloc_chunk_volume, fill_chunk_column, release_chunk_volume } from "@/reference/volume.ts";
import { MaterialClass, NO_LAYER } from "@/render/mesh/voxel.ts";
import { scan_all_column_runs } from "@/reference/voxel_scan.ts";
import { bake_tier, mesh_bounds } from "@/reference/chunk_bake.ts";
import { carve_visibility } from "@/render/mesh/mesh_block.ts";
import { DETACHED_BLOCK } from "@xpute/core/ds/buddy.ts";
import type { BakedChunkMesh } from "@/reference/chunk_mesher.ts";

const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;

/** The frustum of a camera at `eye` looking at `target`, Z up, as its six
 * planes: what the module tests a chunk's bounds against. */
function frustum_of(eye: [number, number, number], target: [number, number, number], fov_deg: number, near: number, far: number): Float64Array {
  const view = mat4_look_at(mat4(), { x: eye[0], y: eye[1], z: eye[2] }, { x: target[0], y: target[1], z: target[2] }, { x: 0, y: 0, z: 1 });
  const proj = mat4_perspective(mat4(), (fov_deg * Math.PI) / 180, 1, near, far);
  return mat4_frustum_planes(new Float64Array(24), mat4_mul(mat4(), proj, view));
}

function frustum_meets_sphere(planes: Float64Array, x: number, y: number, z: number, r: number): boolean {
  for (let k = 0; k < 24; k += 4) if (planes[k] * x + planes[k + 1] * y + planes[k + 2] * z + planes[k + 3] < -r) return false;
  return true;
}

/** A box meets the frustum unless one plane has the whole box behind it. */
function frustum_meets_box(planes: Float64Array, min: [number, number, number], max: [number, number, number]): boolean {
  for (let k = 0; k < 24; k += 4) {
    const px = planes[k] > 0 ? max[0] : min[0], py = planes[k + 1] > 0 ? max[1] : min[1], pz = planes[k + 2] > 0 ? max[2] : min[2];
    if (planes[k] * px + planes[k + 1] * py + planes[k + 2] * pz + planes[k + 3] < 0) return false;
  }
  return true;
}

Deno.test("chunk bake - LOD1 builds usable frustum bounds", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, 0, 0, LAND);
  fill_chunk_column(volume, 49, 2, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const buckets = [bake_tier(column_runs, 1)];
  // Two open-bottom (5-face) terrain boxes: 5*4 = 20 vertices each.
  assertEquals(buckets[0].vertex_cnt, 40);
  assertEquals(mesh_bounds(buckets), {
    min_x: 1,
    min_y: 2,
    min_z: 0,
    max_x: 50,
    max_y: 3,
    max_z: 1,
  });

  // The bounds as a sphere, where a camera above sees it and a thousand
  // units aside does not.
  const b = mesh_bounds(buckets)!;
  const centre: [number, number, number] = [(b.min_x + b.max_x) / 2, (b.min_y + b.max_y) / 2, (b.min_z + b.max_z) / 2];
  const radius = Math.hypot(b.max_x - centre[0], b.max_y - centre[1], b.max_z - centre[2]);
  const frustum = frustum_of([25, 2.5, 80], [25, 2.5, 0], 60, 0.1, 200);
  assert(frustum_meets_sphere(frustum, centre[0], centre[1], centre[2], radius));
  assertEquals(frustum_meets_sphere(frustum, centre[0] + 1000, centre[1], centre[2], radius), false);
});

Deno.test("chunk bake - bounds include signed Z and a second (e.g. object) bucket's own mesh", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 4, 7, -70, -65, LAND);
  const column_runs = scan_all_column_runs(volume);
  const buckets = [bake_tier(column_runs, 1)];

  // Synthetic already-baked mesh spanning a known AABB, standing in for a
  // real placed-object bake — chunk_bounds only ever reads position data,
  // never material/kind, so this isolates its own aggregation from
  // bake_chunk_mesh's real geometry.
  const object_mesh: BakedChunkMesh = {
    block: DETACHED_BLOCK,
    vertex_cnt: 2,
    position: new Float32Array([20, 30, 40, 21, 32, 43]),
    normal: new Float32Array(6),
    local_pos: new Float32Array(6),
    box_scale: new Float32Array(6),
    rgb: new Float32Array(6),
    layer_taps: new Float32Array(8),
    faces: new Float32Array(2),
    index: new Uint32Array(0),
  };
  buckets.push(object_mesh);

  assertEquals(mesh_bounds(buckets), {
    min_x: 4,
    min_y: 7,
    min_z: -70,
    max_x: 21,
    max_y: 32,
    max_z: 43,
  });
});

Deno.test("chunk bake - measured bounds survive a low-polar frustum that rejects the old z=0 proxy", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 31, 31, 200, 200, LAND);
  const column_runs = scan_all_column_runs(volume);
  const bounds = mesh_bounds([bake_tier(column_runs, 1)]);
  assert(bounds !== null);

  const frustum = frustum_of([32, -100, 200.5], [32, 32, 200.5], 50, 0.1, 500);
  assert(frustum_meets_box(frustum, [bounds.min_x, bounds.min_y, bounds.min_z], [bounds.max_x, bounds.max_y, bounds.max_z]));
  assertEquals(frustum_meets_sphere(frustum, 32, 32, 0, 48), false);
});

Deno.test("chunk bake - a fully-covered coarse cell draws one unshrunk box per Z bucket", () => {
  // Two full lod_step=2 blocks (all 4 sub-columns agree, same 2-cell-tall
  // run) at opposite ends of the chunk — each should land in exactly one Z
  // bucket at full (unshrunk) size: coverage == lod_step^3 exactly.
  const volume = alloc_chunk_volume();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) fill_chunk_column(volume, cx, cy, 4, 5, LAND);
  for (const [cx, cy] of [[48, 0], [49, 0], [48, 1], [49, 1]] as const) fill_chunk_column(volume, cx, cy, 4, 5, LAND);
  const column_runs = scan_all_column_runs(volume);

  const bucket = bake_tier(column_runs, 2);
  // Two coarse boxes, each open-bottom (5 faces * 4 verts = 20).
  assertEquals(bucket.vertex_cnt, 40);
  const bounds = mesh_bounds([bucket]);
  assert(bounds !== null);
  assertEquals(bounds.min_z, 4);
  assertEquals(bounds.max_z, 6); // span(2), full coverage -> no isotropic shrink
});

Deno.test("chunk bake - a sparse coarse cell shrinks about its own centre, equally on every axis", () => {
  // 1 of this lod_step=4 cell's 64 fine voxels, so fill is 1/64 and the
  // box's linear scale is its cube root: a 1x1x1 box centred in the cell.
  // Where in the cell the voxel actually sat does not enter into it — the
  // cell is the unit at this tier, and it is a cube.
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const bucket = bake_tier(column_runs, 4);
  const bounds = mesh_bounds([bucket]);
  assert(bounds !== null);
  assertEquals([bounds.min_x, bounds.min_y, bounds.min_z], [1.5, 1.5, 1.5]);
  assertEquals([bounds.max_x, bounds.max_y, bounds.max_z], [2.5, 2.5, 2.5]);
});

Deno.test("chunk bake - a half-full coarse cell shrinks in Z exactly as much as in X and Y", () => {
  // All 4 sub-columns of a lod_step=2 cell, one cell of the two tall: half
  // full, so every axis scales by cbrt(0.5). Height is not spared — that is
  // what keeps the operator the same under a rotation.
  const volume = alloc_chunk_volume();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) fill_chunk_column(volume, cx, cy, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const bucket = bake_tier(column_runs, 2);
  const bounds = mesh_bounds([bucket]);
  assert(bounds !== null);
  const side = 2 * Math.cbrt(0.5);
  // The scale is stored as a 1/256 fraction, so it lands within one step.
  const q = 2 / 256;
  assertAlmostEquals(bounds.max_x - bounds.min_x, side, q);
  assertAlmostEquals(bounds.max_y - bounds.min_y, side, q);
  assertAlmostEquals(bounds.max_z - bounds.min_z, side, q);
  // Whatever that value is, all three axes got exactly it.
  assertEquals(bounds.max_x - bounds.min_x, bounds.max_z - bounds.min_z);
});

Deno.test("chunk bake - column_runs survive the volume they were scanned from being released", () => {
  const volume = alloc_chunk_volume();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) fill_chunk_column(volume, cx, cy, 4, 5, LAND);
  const column_runs = scan_all_column_runs(volume);
  release_chunk_volume(volume);

  const bucket = bake_tier(column_runs, 2);
  assertEquals(bucket.vertex_cnt, 20);
});

/** Every span of a baked mesh is a view into one pooled block, so a wrong
 * offset does not fail loudly — it silently makes two attributes alias.
 * This pins the layout by writing a sentinel through one span and checking
 * every other one is untouched. */
Deno.test("chunk bake - the pooled block's spans do not alias each other", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 3, 3, 0, 2, LAND);
  const column_runs = scan_all_column_runs(volume);
  const mesh = bake_tier(column_runs, 1);

  const spans = [mesh.position, mesh.normal, mesh.local_pos, mesh.box_scale, mesh.rgb, mesh.layer_taps, mesh.faces, carve_visibility(mesh)];
  for (const span of spans) {
    const others = spans.filter((s) => s !== span).map((s) => Array.from(s));
    span.fill(-12345);
    for (const [i, other] of spans.filter((s) => s !== span).entries()) {
      assertEquals(Array.from(other), others[i]);
    }
  }
  assert(mesh.index.every((v: number) => v < mesh.vertex_cnt));
});
