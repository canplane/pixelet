import { assert, assertEquals } from "@std/assert";
import * as THREE from "three";

import { alloc_chunk_volume, fill_chunk_column, MaterialClass, NO_LAYER, release_chunk_volume } from "./volume.ts";
import { build_chunk_surface } from "./chunk_mesher.ts";
import { build_chunk_buckets, build_chunk_bucket_from_surface, chunk_bounds } from "./chunk_bucket.ts";

const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;

Deno.test("chunk bucket - direct volume builds both LOD roles and usable frustum bounds", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, 0, 0, LAND);
  fill_chunk_column(volume, 49, 2, 0, 0, LAND);

  const buckets = build_chunk_buckets(volume, 1, 2);
  assertEquals(buckets.map((bucket) => bucket.lod_step), [1, 2]);
  assertEquals(buckets[0].terrain.cnt, 2);
  assertEquals(Array.from(buckets[0].terrain.matrices.slice(12, 15)), [1.5, 2.5, 0.5]);
  assertEquals(Array.from(buckets[0].terrain.matrices.slice(28, 31)), [49.5, 2.5, 0.5]);
  assertEquals(Array.from(buckets[0].terrain.visibility), [1, 1]);
  assertEquals(Array.from(buckets[1].terrain.visibility), [0, 0]);
  assertEquals(buckets.map((bucket) => bucket.role), ["primary", "secondary"]);
  assertEquals(chunk_bounds(buckets), {
    min_x: 0,
    min_y: 2,
    min_z: -128,
    max_x: 50,
    max_y: 4,
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
  const buckets = build_chunk_buckets(volume, 1, null);
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
  const bounds = chunk_bounds(build_chunk_buckets(volume, 1, null));
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

Deno.test("chunk bucket - retained 36 KiB surface rebuilds coarse LOD after dense bands are released", () => {
  const volume = alloc_chunk_volume();
  fill_chunk_column(volume, 1, 2, -70, 12, LAND);
  fill_chunk_column(volume, 49, 2, -70, 27, LAND);
  const surface = build_chunk_surface(volume);
  assertEquals(surface.z.byteLength + surface.material.byteLength + surface.layer_idx.byteLength, 64 * 64 * 9);

  release_chunk_volume(volume);
  const bucket = build_chunk_bucket_from_surface(surface, 2, "primary");
  assertEquals(bucket.terrain.cnt, 2);
  assertEquals(bucket.objects.cnt, 0);
  assertEquals(bucket.role, "primary");
  assertEquals(Array.from(bucket.terrain.visibility), [1, 1]);
});
