// @pxl/web/render/layer/pipeline/pipeline_integration.test.ts

import { assertEquals } from "@std/assert";
import * as THREE from "three";

import { alloc_chunk_volume, band_material, band_voxel_idx, chunk_band_zs, fill_chunk_column, get_chunk_band, MaterialClass, NO_LAYER, pack_material, set_chunk_voxel } from "./volume.ts";
import { build_chunk_instances, build_chunk_surface, type BuiltChunk } from "./chunk_mesher.ts";
import { set_surface_pixel } from "./chunk_surface.ts";
import { composite_chunk_layers, composite_chunk_surface_layers } from "./compositor.ts";
import { ray_hit_chunk } from "./chunk_raycast.ts";
import type { ChunkKey, WorldLayer } from "./contract.ts";

const KEY: ChunkKey = { page_idx: 0n, page_off: 0n };
const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;
const OBJECT = { material_class: MaterialClass.OBJECT, r: 200, g: 100, b: 50, layer_idx: NO_LAYER } as const;

function bounds_of(built: BuiltChunk): { box: THREE.Box3; sphere: THREE.Sphere } {
  const geometry = new THREE.BoxGeometry(1, 1, 1);
  const material = new THREE.MeshBasicMaterial();
  const mesh = new THREE.InstancedMesh(geometry, material, built.cnt);
  mesh.instanceMatrix = new THREE.InstancedBufferAttribute(built.matrices, 16);
  mesh.computeBoundingBox();
  mesh.computeBoundingSphere();
  const box = mesh.boundingBox!.clone();
  const sphere = mesh.boundingSphere!.clone();
  geometry.dispose();
  material.dispose();
  return { box, sphere };
}

Deno.test("pipeline - composition, signed-band meshing, bounds, and raycast share exact boxes", () => {
  const terrain: WorldLayer = {
    name: "test-terrain",
    revision: () => [1],
    paint: (_key, target) => {
      fill_chunk_column(target, 2, 3, -18, 18, LAND);
      return true;
    },
    paint_surface: (_key, target) => {
      set_surface_pixel(target, 2, 3, 18, pack_material(LAND), LAND.layer_idx);
      return true;
    },
  };
  const objects: WorldLayer = {
    name: "test-objects",
    revision: () => [1],
    paint: (_key, target) => {
      // Exact 1/256 values: x=.25, y=.5, z=.75, size=.5 cell.
      set_chunk_voxel(target, 2, 3, 20, OBJECT, [64, 128, 192, 128]);
      return true;
    },
    paint_surface: (_key, target) => {
      set_surface_pixel(target, 2, 3, 20, pack_material(OBJECT), OBJECT.layer_idx);
      return true;
    },
  };

  const volume = composite_chunk_layers([terrain, objects], KEY)!;
  const dense_surface = build_chunk_surface(volume);
  const direct_surface = composite_chunk_surface_layers([terrain, objects], KEY)!;
  assertEquals(direct_surface, dense_surface);
  assertEquals(chunk_band_zs(volume), [-1, 0]);

  const built = build_chunk_instances(volume, 1);
  assertEquals([built.terrain.cnt, built.objects.cnt], [1, 1]);
  assertEquals(Array.from(built.terrain.matrices.slice(12, 15)), [2.5, 3.5, 0.5]);
  assertEquals(Array.from(built.objects.matrices.slice(12, 15)), [2.5, 3.75, 21]);

  const terrain_bounds = bounds_of(built.terrain);
  const object_bounds = bounds_of(built.objects);
  assertEquals(terrain_bounds.box.min.toArray(), [2, 3, -18]);
  assertEquals(terrain_bounds.box.max.toArray(), [3, 4, 19]);
  assertEquals(object_bounds.box.min.toArray(), [2.25, 3.5, 20.75]);
  assertEquals(object_bounds.box.max.toArray(), [2.75, 4, 21.25]);
  assertEquals(Number.isFinite(terrain_bounds.sphere.radius) && terrain_bounds.sphere.radius > 0, true);
  assertEquals(Number.isFinite(object_bounds.sphere.radius) && object_bounds.sphere.radius > 0, true);

  const down = ray_hit_chunk(new THREE.Ray(new THREE.Vector3(2.5, 3.75, 30), new THREE.Vector3(0, 0, -1)), volume, 0, 0, 100)!;
  assertEquals(down.material_class, MaterialClass.OBJECT);
  assertEquals(down.face_normal, [0, 0, 1]);
  assertEquals(down.hit_point, [2.5, 3.75, 21.25]);
  assertEquals(down.box_min, object_bounds.box.min.toArray());
  assertEquals(down.box_max, object_bounds.box.max.toArray());

  const side = ray_hit_chunk(new THREE.Ray(new THREE.Vector3(10, 3.25, 17), new THREE.Vector3(-1, 0, 0)), volume, 0, 0, 100)!;
  assertEquals(side.material_class, MaterialClass.LAND);
  assertEquals(side.face_normal, [1, 0, 0]);
  assertEquals(side.hit_point, [3, 3.25, 17]);
  assertEquals(side.box_min, terrain_bounds.box.min.toArray());
  assertEquals(side.box_max, terrain_bounds.box.max.toArray());
});

Deno.test("pipeline - one dense band stores direct 5-byte materials without a palette", () => {
  const volume = alloc_chunk_volume();
  for (let i = 0; i < 256; i++) {
    set_chunk_voxel(volume, i & 63, i >> 6, 0, {
      material_class: MaterialClass.LAND,
      r: i,
      g: 0,
      b: 0,
      layer_idx: NO_LAYER,
    });
  }

  const band = get_chunk_band(volume, 0, 0, 0)!;
  assertEquals(band.material.byteLength + band.layer_idx.byteLength, 64 * 64 * 64 * 5);
  const last_idx = band_voxel_idx(63, 3, 0);
  assertEquals(band_material(band, last_idx).r, 255);
});
