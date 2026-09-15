// @pxl/web/reference/compositor.test.ts

import { assertEquals } from "@std/assert";

import { band_material, band_voxel_idx, get_chunk_band, set_chunk_voxel } from "@/reference/volume.ts";
import { MaterialClass, NO_LAYER } from "@/reference/mesh/voxel.ts";
import { composite_chunk_layers } from "@/reference/compositor.ts";
import type { ChunkKey, WorldLayer } from "@/reference/contract.ts";

const KEY: ChunkKey = { page_idx: 0n, page_off: 0n };

function layer(name: string, r: number): WorldLayer {
  return {
    name,
    paint: (_key, target) => {
      set_chunk_voxel(target, 0, 0, 0, { material_class: MaterialClass.OBJECT, r, g: 0, b: 0, layer_idx: NO_LAYER });
      return true;
    },
  };
}

Deno.test("compositor - later registered layers overwrite the same voxel", () => {
  const volume = composite_chunk_layers([layer("first", 10), layer("second", 20)], KEY)!;
  const band = get_chunk_band(volume, 0)!;
  assertEquals(band_material(band, band_voxel_idx(0, 0, 0)).r, 20);
});

Deno.test("compositor - explicit air from a later layer clears lower terrain", () => {
  const air: WorldLayer = {
    name: "air",
    paint: (_key, target) => {
      set_chunk_voxel(target, 0, 0, 0, { material_class: MaterialClass.AIR, r: 0, g: 0, b: 0, layer_idx: NO_LAYER });
      return true;
    },
  };
  const volume = composite_chunk_layers([layer("terrain", 10), air], KEY)!;
  const band = get_chunk_band(volume, 0)!;
  assertEquals(band_material(band, band_voxel_idx(0, 0, 0)).material_class, MaterialClass.AIR);
});
