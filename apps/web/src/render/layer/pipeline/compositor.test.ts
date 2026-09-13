// @pxl/web/render/layer/pipeline/compositor.test.ts

import { assertEquals } from "@std/assert";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { band_material, band_voxel_idx, get_chunk_band, set_chunk_voxel } from "./volume.ts";
import { MaterialClass, NO_LAYER } from "./voxel.ts";
import { clear_compositor_cache, composite_chunk_layers, compositor_cache_stats, same_layer_revisions, with_raycast_composite } from "./compositor.ts";
import type { ChunkKey, WorldLayer } from "./contract.ts";

const KEY: ChunkKey = { page_idx: 0n, page_off: 0n };

function layer(name: string, r: number): WorldLayer {
  return {
    name,
    revision: () => [r],
    paint: (_key, target) => {
      set_chunk_voxel(target, 0, 0, 0, { material_class: MaterialClass.OBJECT, r, g: 0, b: 0, layer_idx: NO_LAYER });
      return true;
    },
  };
}

Deno.test("compositor - later registered layers overwrite the same voxel", () => {
  const volume = composite_chunk_layers([layer("first", 10), layer("second", 20)], KEY)!;
  const band = get_chunk_band(volume, 0, 0, 0)!;
  assertEquals(band_material(band, band_voxel_idx(0, 0, 0)).r, 20);
});

Deno.test("compositor - explicit air from a later layer clears lower terrain", () => {
  const air: WorldLayer = {
    name: "air",
    revision: () => [1],
    paint: (_key, target) => {
      set_chunk_voxel(target, 0, 0, 0, { material_class: MaterialClass.AIR, r: 0, g: 0, b: 0, layer_idx: NO_LAYER });
      return true;
    },
  };
  const volume = composite_chunk_layers([layer("terrain", 10), air], KEY)!;
  const band = get_chunk_band(volume, 0, 0, 0)!;
  assertEquals(band_material(band, band_voxel_idx(0, 0, 0)).material_class, MaterialClass.AIR);
});

Deno.test("compositor - revision tuples compare each layer-owned dependency", () => {
  const stable = {};
  assertEquals(same_layer_revisions([[stable, true], [4]], [[stable, true], [4]]), true);
  assertEquals(same_layer_revisions([[stable, true], [4]], [[{}, true], [4]]), false);
  assertEquals(same_layer_revisions([[stable, true], [4]], [[stable, false], [4]]), false);
});

Deno.test("compositor - unchanged revisions reuse the cached field and changed revisions repaint it", () => {
  let revision: object = {};
  let paints = 0;
  const cached_layer: WorldLayer = {
    name: "cached",
    revision: () => [revision],
    paint: (_key, target) => {
      paints++;
      set_chunk_voxel(target, 0, 0, 0, { material_class: MaterialClass.LAND, r: paints, g: 0, b: 0, layer_idx: NO_LAYER });
      return true;
    },
  };
  const stack = [cached_layer] as const;
  const key: ChunkKey = { page_idx: 123n, page_off: 456n };

  const first = with_raycast_composite(stack, key, (volume) => volume);
  const cached = with_raycast_composite(stack, key, (volume) => volume);
  assertEquals(cached === first, true);
  assertEquals(paints, 1);

  revision = {};
  const repainted = with_raycast_composite(stack, key, (volume) => volume);
  assertEquals(repainted === first, false);
  assertEquals(paints, 2);
});

Deno.test("compositor - ray LRU retains at most four recent dense volumes", () => {
  clear_compositor_cache();
  const stack = [layer("bounded", 1)] as const;
  for (let x = 0; x < 5; x++) {
    const key: ChunkKey = { page_idx: 0n, page_off: ivec.gather(BigInt(x * 64), 0n, 0n) };
    with_raycast_composite(stack, key, () => undefined);
  }
  const stats = compositor_cache_stats();
  assertEquals(stats.entries, 4);
  assertEquals(stats.bands, 4);
  clear_compositor_cache();
});
