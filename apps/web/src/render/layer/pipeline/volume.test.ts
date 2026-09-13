// @pxl/web/render/layer/pipeline/volume.test.ts

import { assert, assertEquals } from "@std/assert";

import { CORE_POOL } from "./core_pool.ts";
import { alloc_chunk_volume, band_voxel_idx, get_chunk_band, L2_BAND_VOXEL_CNT, release_chunk_volume, set_chunk_voxel } from "./volume.ts";
import { MaterialClass, NO_LAYER } from "./voxel.ts";

const STONE = { material_class: MaterialClass.OBJECT, r: 9, g: 9, b: 9, layer_idx: NO_LAYER };

Deno.test("volume - a band's sub-cell buffer goes back to the core pool with the volume", () => {
  const volume = alloc_chunk_volume();
  set_chunk_voxel(volume, 0, 0, 0, STONE);
  const before = CORE_POOL.stat.live_bytes;
  set_chunk_voxel(volume, 1, 0, 0, STONE, [10, 20, 30, 128]);
  assertEquals(CORE_POOL.stat.live_bytes - before, L2_BAND_VOXEL_CNT * 4, "one sub-cell block borrowed");
  release_chunk_volume(volume);
  assert(CORE_POOL.stat.live_bytes <= before, "and returned; the band itself is pooled or returned too");
});

Deno.test("volume - a sub-cell buffer borrowed after another's reads as none everywhere it was not written", () => {
  // Pool blocks are not cleared between borrowers. A sub-cell buffer that
  // kept the last one's offsets would give plain voxels a sub-cell shape.
  const first = alloc_chunk_volume();
  set_chunk_voxel(first, 5, 5, 5, STONE, [200, 200, 200, 200]);
  release_chunk_volume(first);

  const second = alloc_chunk_volume();
  set_chunk_voxel(second, 9, 9, 9, STONE, [1, 2, 3, 4]);
  const band = get_chunk_band(second, 0)!;
  const stale = band_voxel_idx(5, 5, 5) * 4;
  assertEquals([...band.sub_cell!.subarray(stale, stale + 4)], [0, 0, 0, 0]);
  release_chunk_volume(second);
});
