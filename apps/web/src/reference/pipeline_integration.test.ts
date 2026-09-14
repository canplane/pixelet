// @pxl/web/reference/pipeline_integration.test.ts

import { assertEquals } from "@std/assert";

import { alloc_chunk_volume, band_material, band_voxel_idx, chunk_band_zs, fill_chunk_column, get_chunk_band, set_chunk_voxel } from "@/reference/volume.ts";
import { MaterialClass, NO_LAYER } from "@/render/mesh/voxel.ts";
import { DETACHED_BLOCK } from "@xpute/core/ds/buddy.ts";
import { bake_chunk_mesh, type BakedChunkMesh } from "@/reference/chunk_mesher.ts";
import { CORE_BAKE } from "@/reference/chunk_bake.ts";
import { scan_all_column_runs } from "@/reference/voxel_scan.ts";
import { composite_chunk_layers } from "@/reference/compositor.ts";
import { ray_hit_chunk } from "@/reference/chunk_raycast.ts";
import type { ChunkKey, WorldLayer } from "@/reference/contract.ts";

const KEY: ChunkKey = { page_idx: 0n, page_off: 0n };
const LAND = { material_class: MaterialClass.LAND, r: 10, g: 20, b: 30, layer_idx: NO_LAYER } as const;
const OBJECT = { material_class: MaterialClass.OBJECT, r: 200, g: 100, b: 50, layer_idx: NO_LAYER } as const;

type Bounds = { min: [number, number, number]; max: [number, number, number]; center: [number, number, number]; radius: number };

/** Real bounding box and sphere off a baked mesh's own vertex positions. */
function bounds_of(built: BakedChunkMesh): Bounds {
  const min: [number, number, number] = [Infinity, Infinity, Infinity];
  const max: [number, number, number] = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < built.vertex_cnt; i++) {
    for (let k = 0; k < 3; k++) {
      const v = built.position[i * 3 + k];
      if (v < min[k]) min[k] = v;
      if (v > max[k]) max[k] = v;
    }
  }
  const center: [number, number, number] = [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2];
  let radius = 0;
  for (let i = 0; i < built.vertex_cnt; i++) {
    radius = Math.max(radius, Math.hypot(built.position[i * 3] - center[0], built.position[i * 3 + 1] - center[1], built.position[i * 3 + 2] - center[2]));
  }
  return { min, max, center, radius };
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
    paint: (_key, target) => {
      fill_chunk_column(target, 2, 3, -18, 18, LAND);
      return true;
    },
  };
  const objects: WorldLayer = {
    name: "test-objects",
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
  assertEquals(terrain_bounds.center, [2.5, 3.5, 0.5]);
  assertEquals(object_bounds.center, [2.5, 3.75, 21]);
  assertEquals(terrain_bounds.min, [2, 3, -18]);
  assertEquals(terrain_bounds.max, [3, 4, 19]);
  assertEquals(object_bounds.min, [2.25, 3.5, 20.75]);
  assertEquals(object_bounds.max, [2.75, 4, 21.25]);
  assertEquals(Number.isFinite(terrain_bounds.radius) && terrain_bounds.radius > 0, true);
  assertEquals(Number.isFinite(object_bounds.radius) && object_bounds.radius > 0, true);

  // ray_hit_chunk still reads the live volume directly (merge_column_runs
  // per column, on demand) — a separate, unrelated-to-column_runs path, so
  // it has to agree with build_chunk_instances' own boxes independently.
  const down = ray_hit_chunk({ origin: { x: 2.5, y: 3.75, z: 30 }, direction: { x: 0, y: 0, z: -1 } }, volume, 0, 0, 100)!;
  assertEquals(down.material_class, MaterialClass.OBJECT);
  assertEquals(down.face_normal, [0, 0, 1]);
  assertEquals(down.hit_point, [2.5, 3.75, 21.25]);
  assertEquals(down.box_min, object_bounds.min);
  assertEquals(down.box_max, object_bounds.max);

  const side = ray_hit_chunk({ origin: { x: 10, y: 3.25, z: 17 }, direction: { x: -1, y: 0, z: 0 } }, volume, 0, 0, 100)!;
  assertEquals(side.material_class, MaterialClass.LAND);
  assertEquals(side.face_normal, [1, 0, 0]);
  assertEquals(side.hit_point, [3, 3.25, 17]);
  assertEquals(side.box_min, terrain_bounds.min);
  assertEquals(side.box_max, terrain_bounds.max);
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
