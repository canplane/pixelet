// @pxl/web/render/layer/pipeline/chunk_bucket.test.ts

import { assert, assertEquals } from "@std/assert";
import * as THREE from "three";

import { alloc_chunk_volume, fill_chunk_column, MaterialClass, NO_LAYER, release_chunk_volume } from "./volume.ts";
import { scan_all_column_runs } from "./voxel_scan.ts";
import { build_chunk_bucket, build_chunk_buckets, chunk_bounds } from "./chunk_bucket.ts";

const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;

Deno.test("chunk bucket - LOD1 builds usable frustum bounds", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, 0, 0, LAND);
  fill_chunk_column(volume, 49, 2, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const buckets = build_chunk_buckets(column_runs, 1, null);
  assertEquals(buckets.map((bucket) => bucket.lod_step), [1]);
  assertEquals(buckets[0].terrain.cnt, 2);
  assertEquals(Array.from(buckets[0].terrain.matrices.slice(12, 15)), [1.5, 2.5, 0.5]);
  assertEquals(Array.from(buckets[0].terrain.matrices.slice(28, 31)), [49.5, 2.5, 0.5]);
  assertEquals(Array.from(buckets[0].terrain.visibility), [1, 1]);
  assertEquals(buckets.map((bucket) => bucket.role), ["primary"]);
  assertEquals(chunk_bounds(buckets), {
    min_x: 1,
    min_y: 2,
    min_z: 0,
    max_x: 50,
    max_y: 3,
    max_z: 1,
  });

  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const material = new THREE.MeshBasicMaterial();
  const mesh = new THREE.InstancedMesh(geometry, material, buckets[0].terrain.cnt);
  mesh.instanceMatrix = new THREE.InstancedBufferAttribute(buckets[0].terrain.matrices, 16);
  mesh.computeBoundingBox();
  mesh.computeBoundingSphere();
  assertEquals(mesh.boundingBox!.min.toArray(), [1, 2, 0]);
  assertEquals(mesh.boundingBox!.max.toArray(), [50, 3, 1]);

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

Deno.test("chunk bucket - bounds include signed Z and object instances", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 4, 7, -70, -65, LAND);
  const column_runs = scan_all_column_runs(volume);
  const buckets = build_chunk_buckets(column_runs, 1, null);
  const object = buckets[0].objects;
  object.cnt = 1;
  object.matrices = new Float32Array([
    2,
    0,
    0,
    0,
    0,
    4,
    0,
    0,
    0,
    0,
    6,
    0,
    20,
    30,
    40,
    1,
  ]);

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
  const bounds = chunk_bounds(build_chunk_buckets(column_runs, 1, null));
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
  assertEquals(bucket.terrain.cnt, 2);
  const span = 2; // lod_step * U_PER_CELL
  assertEquals(bucket.terrain.matrices[10], span); // full coverage -> no isotropic shrink
  assertEquals(bucket.terrain.matrices[26], span);
  assertEquals(Array.from(bucket.terrain.matrices.slice(12, 15)), [1, 1, 5]);
  assertEquals(Array.from(bucket.terrain.matrices.slice(28, 31)), [49, 1, 5]);
});

Deno.test("chunk bucket - a partially-covered coarse cell shrinks isotropically instead of vanishing", () => {
  // Only 1 of the 4 sub-columns in this lod_step=2 cell is filled: fill =
  // 1/8, so the box shrinks to cbrt(1/8) = 0.5 of the cell's full span on
  // every axis (X/Y/Z together) rather than being dropped by a threshold.
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, 0, 0, LAND);
  const column_runs = scan_all_column_runs(volume);

  const bucket = build_chunk_bucket(column_runs, 2, "primary");
  assertEquals(bucket.terrain.cnt, 1);
  assertEquals(bucket.terrain.matrices[0], 1); // X scale: span(2) * cbrt(1/8)(0.5)
  assertEquals(bucket.terrain.matrices[5], 1); // Y scale
  assertEquals(bucket.terrain.matrices[10], 1); // Z scale — same factor, isotropic
});

Deno.test("chunk bucket - column_runs survive the volume they were scanned from being released", () => {
  const volume = alloc_chunk_volume();
  for (const [cx, cy] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) fill_chunk_column(volume, cx, cy, 4, 5, LAND);
  const column_runs = scan_all_column_runs(volume);
  release_chunk_volume(volume);

  const bucket = build_chunk_bucket(column_runs, 2, "primary");
  assertEquals(bucket.terrain.cnt, 1);
  assertEquals(bucket.objects.cnt, 0);
  assertEquals(bucket.role, "primary");
  assertEquals(Array.from(bucket.terrain.visibility), [1]);
  assertEquals(Array.from(bucket.terrain.matrices.slice(12, 15)), [1, 1, 5]);
});
