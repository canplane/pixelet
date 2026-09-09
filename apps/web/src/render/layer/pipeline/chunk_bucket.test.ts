// @pxl/web/render/layer/pipeline/chunk_bucket.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";
import * as THREE from "three";

import { alloc_chunk_volume, fill_chunk_column, MaterialClass, NO_LAYER, release_chunk_volume } from "./volume.ts";
import { scan_all_column_runs } from "./voxel_scan.ts";
import { build_chunk_bucket, chunk_bounds, release_built_mesh, retain_built_mesh } from "./chunk_bucket.ts";
import { DETACHED_BLOCK } from "@xpute/core/ds/buddy.ts";
import type { BakedChunkMesh } from "./chunk_mesher.ts";

const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;

/** Builds a real, renderer-facing BufferGeometry from one baked mesh — the
 * same attribute wiring chunk_stage.tsx's own create_geometry does — so a
 * test can hand it to THREE's own frustum/bounding-volume machinery, same
 * as this file always has. */
function geometry_of(built: BakedChunkMesh): THREE.BufferGeometry {
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(built.position, 3));
  geo.setIndex(new THREE.BufferAttribute(built.index, 1));
  return geo;
}

Deno.test("chunk bucket - LOD1 builds usable frustum bounds", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, 0, 0, LAND);
  fill_chunk_column(volume, 49, 2, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const buckets = [build_chunk_bucket(column_runs, 1, "primary")];
  assertEquals(buckets.map((bucket) => bucket.lod_step), [1]);
  // Two open-bottom (5-face) terrain boxes: 5*4 = 20 vertices each.
  assertEquals(buckets[0].cubes.vertex_cnt, 40);
  assertEquals(Array.from(buckets[0].cubes.visibility), new Array(40).fill(1));
  assertEquals(buckets.map((bucket) => bucket.role), ["primary"]);
  assertEquals(chunk_bounds(buckets), {
    min_x: 1,
    min_y: 2,
    min_z: 0,
    max_x: 50,
    max_y: 3,
    max_z: 1,
  });

  const geometry = geometry_of(buckets[0].cubes);
  const material = new THREE.MeshBasicMaterial();
  const mesh = new THREE.Mesh(geometry, material);
  geometry.computeBoundingBox();
  geometry.computeBoundingSphere();
  assertEquals(geometry.boundingBox!.min.toArray(), [1, 2, 0]);
  assertEquals(geometry.boundingBox!.max.toArray(), [50, 3, 1]);

  const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 200);
  camera.position.set(25, 2.5, 80);
  camera.lookAt(25, 2.5, 0);
  camera.updateMatrixWorld(true);
  camera.updateProjectionMatrix();
  const projection_view = new THREE.Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
  const frustum = new THREE.Frustum().setFromProjectionMatrix(projection_view);
  mesh.updateMatrixWorld(true);
  assert(frustum.intersectsObject(mesh));
  mesh.position.x = 1000;
  mesh.updateMatrixWorld(true);
  assertEquals(frustum.intersectsObject(mesh), false);

  geometry.dispose();
  material.dispose();
});

Deno.test("chunk bucket - bounds include signed Z and a second (e.g. object) bucket's own mesh", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 4, 7, -70, -65, LAND);
  const column_runs = scan_all_column_runs(volume);
  const buckets = [build_chunk_bucket(column_runs, 1, "primary")];

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
  buckets.push({ lod_step: 1, role: "secondary", cubes: { ...object_mesh, visibility: new Float32Array(2).fill(1), rc: 1 } });

  assertEquals(chunk_bounds(buckets), {
    min_x: 4,
    min_y: 7,
    min_z: -70,
    max_x: 21,
    max_y: 32,
    max_z: 43,
  });
});

Deno.test("chunk bucket - measured bounds survive a low-polar frustum that rejects the old z=0 proxy", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 31, 31, 200, 200, LAND);
  const column_runs = scan_all_column_runs(volume);
  const bounds = chunk_bounds([build_chunk_bucket(column_runs, 1, "primary")]);
  assert(bounds !== null);

  const camera = new THREE.PerspectiveCamera(50, 1, 0.1, 500);
  camera.position.set(32, -100, 200.5);
  camera.lookAt(32, 32, 200.5);
  camera.updateMatrixWorld(true);
  camera.updateProjectionMatrix();
  const projection_view = new THREE.Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
  const frustum = new THREE.Frustum().setFromProjectionMatrix(projection_view);
  const measured = new THREE.Box3(
    new THREE.Vector3(bounds.min_x, bounds.min_y, bounds.min_z),
    new THREE.Vector3(bounds.max_x, bounds.max_y, bounds.max_z),
  );

  assert(frustum.intersectsBox(measured));
  assertEquals(frustum.intersectsSphere(new THREE.Sphere(new THREE.Vector3(32, 32, 0), 48)), false);
});

Deno.test("chunk bucket - a fully-covered coarse cell draws one unshrunk box per Z bucket", () => {
  // Two full lod_step=2 blocks (all 4 sub-columns agree, same 2-cell-tall
  // run) at opposite ends of the chunk — each should land in exactly one Z
  // bucket at full (unshrunk) size: coverage == lod_step^3 exactly.
  const volume = alloc_chunk_volume();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) fill_chunk_column(volume, cx, cy, 4, 5, LAND);
  for (const [cx, cy] of [[48, 0], [49, 0], [48, 1], [49, 1]] as const) fill_chunk_column(volume, cx, cy, 4, 5, LAND);
  const column_runs = scan_all_column_runs(volume);

  const bucket = build_chunk_bucket(column_runs, 2, "primary");
  // Two coarse boxes, each open-bottom (5 faces * 4 verts = 20).
  assertEquals(bucket.cubes.vertex_cnt, 40);
  const bounds = chunk_bounds([bucket]);
  assert(bounds !== null);
  assertEquals(bounds.min_z, 4);
  assertEquals(bounds.max_z, 6); // span(2), full coverage -> no isotropic shrink
});

Deno.test("chunk bucket - a sparse coarse cell shrinks about its own centre, equally on every axis", () => {
  // 1 of this lod_step=4 cell's 64 fine voxels, so fill is 1/64 and the
  // box's linear scale is its cube root: a 1x1x1 box centred in the cell.
  // Where in the cell the voxel actually sat does not enter into it — the
  // cell is the unit at this tier, and it is a cube.
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const bucket = build_chunk_bucket(column_runs, 4, "primary");
  const bounds = chunk_bounds([bucket]);
  assert(bounds !== null);
  assertEquals([bounds.min_x, bounds.min_y, bounds.min_z], [1.5, 1.5, 1.5]);
  assertEquals([bounds.max_x, bounds.max_y, bounds.max_z], [2.5, 2.5, 2.5]);
});

Deno.test("chunk bucket - a half-full coarse cell shrinks in Z exactly as much as in X and Y", () => {
  // All 4 sub-columns of a lod_step=2 cell, one cell of the two tall: half
  // full, so every axis scales by cbrt(0.5). Height is not spared — that is
  // what keeps the operator the same under a rotation.
  const volume = alloc_chunk_volume();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) fill_chunk_column(volume, cx, cy, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const bucket = build_chunk_bucket(column_runs, 2, "primary");
  const bounds = chunk_bounds([bucket]);
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

Deno.test("chunk bucket - column_runs survive the volume they were scanned from being released", () => {
  const volume = alloc_chunk_volume();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) fill_chunk_column(volume, cx, cy, 4, 5, LAND);
  const column_runs = scan_all_column_runs(volume);
  release_chunk_volume(volume);

  const bucket = build_chunk_bucket(column_runs, 2, "primary");
  assertEquals(bucket.cubes.vertex_cnt, 20);
  assertEquals(bucket.role, "primary");
  assertEquals(Array.from(bucket.cubes.visibility), new Array(20).fill(1));
});

/** Every span of a baked mesh is a view into one pooled block, so a wrong
 * offset does not fail loudly — it silently makes two attributes alias.
 * This pins the layout by writing a sentinel through one span and checking
 * every other one is untouched. */
Deno.test("chunk bucket - the pooled block's spans do not alias each other", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 3, 3, 0, 2, LAND);
  const column_runs = scan_all_column_runs(volume);
  const mesh = build_chunk_bucket(column_runs, 1, "primary").cubes;

  const spans = [mesh.position, mesh.normal, mesh.local_pos, mesh.box_scale, mesh.rgb, mesh.layer_taps, mesh.faces, mesh.visibility];
  for (const span of spans) {
    const others = spans.filter((s) => s !== span).map((s) => Array.from(s));
    span.fill(-12345);
    for (const [i, other] of spans.filter((s) => s !== span).entries()) {
      assertEquals(Array.from(other), others[i]);
    }
  }
  assert(mesh.index.every((v) => v < mesh.vertex_cnt));
});

/** Refcount semantics, independent of whether the pool is recycling: a
 * mesh with two owners (the LOD cache and a mounted geometry) must survive
 * the first release and only give its block back on the second. */
Deno.test("chunk bucket - a retained mesh survives one release and not two", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 5, 5, 0, 3, LAND);
  const column_runs = scan_all_column_runs(volume);

  const mesh = build_chunk_bucket(column_runs, 1, "primary").cubes;
  retain_built_mesh(mesh);
  assertEquals(mesh.rc, 2);
  release_built_mesh(mesh);
  assertEquals(mesh.rc, 1);
  assertEquals(mesh.block.free, false);
  release_built_mesh(mesh);
  assertEquals(mesh.rc, 0);
});
