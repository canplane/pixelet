// @pxl/web/render/layer/pipeline/pipeline_integration.test.ts

import { assertEquals } from "@std/assert";
import * as THREE from "three";

import { alloc_chunk_volume, band_material, band_voxel_idx, chunk_band_zs, fill_chunk_column, get_chunk_band, set_chunk_voxel } from "./volume.ts";
import { MaterialClass, NO_LAYER } from "./voxel.ts";
import { DETACHED_BLOCK } from "@xpute/core/ds/buddy.ts";
import { bake_chunk_mesh, type BakedChunkMesh } from "./chunk_mesher.ts";
import { CORE_BAKE } from "./core_build.ts";
import { scan_all_column_runs } from "./voxel_scan.ts";
import { composite_chunk_layers } from "./compositor.ts";
import { ray_hit_chunk } from "./chunk_raycast.ts";
import type { ChunkKey, WorldLayer } from "./contract.ts";

const KEY: ChunkKey = { page_idx: 0n, page_off: 0n };
const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;
const OBJECT = { material_class: MaterialClass.OBJECT, r: 200, g: 100, b: 50, layer_idx: NO_LAYER } as const;

/** Real bounding box/sphere off a baked mesh's own vertex positions — same
 * attribute wiring chunk_stage.tsx's own create_geometry does. */
function bounds_of(built: BakedChunkMesh): { box: THREE.Box3; sphere: THREE.Sphere } {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(built.position, 3));
  geometry.setIndex(new THREE.BufferAttribute(built.index, 1));
  geometry.computeBoundingBox();
  geometry.computeBoundingSphere();
  const box = geometry.boundingBox!.clone();
  const sphere = geometry.boundingSphere!.clone();
  geometry.dispose();
  return { box, sphere };
}

/** Splits one bake_chunk_mesh output back into its terrain-only vs
 * object-only vertex ranges, purely from each vertex's own baked face
 * count (full object boxes bake 24 vertices, open-bottom terrain boxes bake
 * 20) — this test scenario only ever has one box of each, so a single split
 * point is unambiguous. The two halves are named for what they were when
 * terrain came first; the caller reads them in emission order. */
function split_terrain_and_object(built: BakedChunkMesh, terrain_vertex_cnt: number): { terrain: BakedChunkMesh; object: BakedChunkMesh } {
  function slice(from: number, to: number): BakedChunkMesh {
    return {
      block: DETACHED_BLOCK,
      vertex_cnt: to - from,
      position: built.position.slice(from * 3, to * 3),
      normal: built.normal.slice(from * 3, to * 3),
      local_pos: built.local_pos.slice(from * 3, to * 3),
      box_scale: built.box_scale.slice(from * 3, to * 3),
      rgb: built.rgb.slice(from * 3, to * 3),
      layer_taps: built.layer_taps.slice(from * 4, to * 4),
      faces: built.faces.slice(from, to),
      index: new Uint32Array(0),
    };
  }
  return { terrain: slice(0, terrain_vertex_cnt), object: slice(terrain_vertex_cnt, built.vertex_cnt) };
}

Deno.test("pipeline - composition, signed-band meshing, bounds, and raycast share exact boxes", () => {
  const terrain: WorldLayer = {
    name: "test-terrain",
    revision: () => [1],
    paint: (_key, target) => {
      fill_chunk_column(target, 2, 3, -18, 18, LAND);
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
  };

  const volume = composite_chunk_layers([terrain, objects], KEY)!;
  assertEquals(chunk_band_zs(volume), [-1, 0]);

  // scan_all_column_runs (see voxel_scan.ts) is the shared entry point
  // build_chunk_bucket/chunk_stage.tsx use — one full-chunk scan, reused
  // across LOD builds, instead of bake_chunk_mesh touching the volume
  // itself.
  const column_runs = scan_all_column_runs(volume);
  const built = bake_chunk_mesh(CORE_BAKE, column_runs, 1);
  // One full object box (24 verts — OBJECT-class runs always keep their
  // bottom face) + one open-bottom terrain box (20 verts), both in this one
  // baked mesh. The object comes first: the greedy merge emits everything
  // that cannot merge — sub-cell runs, and any run after a column's first —
  // before it walks the mergeable plane. Emission order within a chunk is
  // not otherwise meaningful, it is one draw either way.
  assertEquals(built.vertex_cnt, 24 + 20);
  const split = split_terrain_and_object(built, 24);

  const object_bounds = bounds_of(split.terrain);
  const terrain_bounds = bounds_of(split.object);
  assertEquals(terrain_bounds.box.getCenter(new THREE.Vector3()).toArray(), [2.5, 3.5, 0.5]);
  assertEquals(object_bounds.box.getCenter(new THREE.Vector3()).toArray(), [2.5, 3.75, 21]);
  assertEquals(terrain_bounds.box.min.toArray(), [2, 3, -18]);
  assertEquals(terrain_bounds.box.max.toArray(), [3, 4, 19]);
  assertEquals(object_bounds.box.min.toArray(), [2.25, 3.5, 20.75]);
  assertEquals(object_bounds.box.max.toArray(), [2.75, 4, 21.25]);
  assertEquals(Number.isFinite(terrain_bounds.sphere.radius) && terrain_bounds.sphere.radius > 0, true);
  assertEquals(Number.isFinite(object_bounds.sphere.radius) && object_bounds.sphere.radius > 0, true);

  // ray_hit_chunk still reads the live volume directly (merge_column_runs
  // per column, on demand) — a separate, unrelated-to-column_runs path, so
  // it has to agree with build_chunk_instances' own boxes independently.
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

  const band = get_chunk_band(volume, 0)!;
  assertEquals(band.material.byteLength + band.layer_idx.byteLength, 64 * 64 * 64 * 5);
  const last_idx = band_voxel_idx(63, 3, 0);
  assertEquals(band_material(band, last_idx).r, 255);
});
