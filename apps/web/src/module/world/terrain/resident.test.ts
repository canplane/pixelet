// @pxl/web/module/world/terrain/resident.test.ts

import { assert, assertEquals } from "@std/assert";

import { GroundChannelKey, type Terrain } from "@pxl/shared/bake/format.ts";

import { CORE_MEMORY, PAGE_POOL } from "@/wasm/core_pool.ts";
import { into_core, release_terrain } from "./resident.ts";

const CHANNEL_BYTES = 1 << 20;

function decoded_page(seed: number): Terrain {
  const channel = (k: number) => new Uint8Array(CHANNEL_BYTES).fill((seed + k) & 0xff);
  return {
    ele: new Int16Array(68 * 68).fill(seed),
    ground: {
      [GroundChannelKey.LAND]: channel(1),
      [GroundChannelKey.WATER]: channel(2),
      [GroundChannelKey.STRUCTURE]: channel(3),
      [GroundChannelKey.TRANSPORT]: channel(4),
    },
  } as Terrain;
}

const settle = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

// Each test waits out the quarantine of what it released, so no release
// timer outlives the test that started it and lands in the next one's count.

Deno.test("into_core - a page's arrays are copied into the core memory, value for value", async () => {
  const src = decoded_page(7);
  const page = into_core(src);
  try {
    assertEquals(page.ele[0], 7);
    assertEquals(page.ground[GroundChannelKey.TRANSPORT][CHANNEL_BYTES - 1], 11);
    if (CORE_MEMORY) {
      assert(page.ele.buffer === CORE_MEMORY.buffer, "elevation lives in the core memory");
      assert(page.ground[GroundChannelKey.LAND].buffer === CORE_MEMORY.buffer, "channels live in the core memory");
    }
    assert(page.ground[GroundChannelKey.LAND].buffer !== src.ground[GroundChannelKey.LAND].buffer, "the response buffer is not kept");
  } finally {
    release_terrain(page, 0);
    await settle(10);
  }
});

Deno.test("into_core - each channel takes exactly one 1 MiB block, not a rounded-up page", async () => {
  const before = PAGE_POOL.stat.live_blocks;
  const page = into_core(decoded_page(1));
  assertEquals(PAGE_POOL.stat.live_blocks - before, 5, "four channels and the elevation grid");
  release_terrain(page, 0);
  await settle(10);
});

Deno.test("release_terrain - blocks go back only after the quarantine", async () => {
  const before = PAGE_POOL.stat.live_bytes;
  const page = into_core(decoded_page(2));
  const held = PAGE_POOL.stat.live_bytes - before;
  release_terrain(page, 20);
  assertEquals(PAGE_POOL.stat.live_bytes - before, held, "still borrowed during the quarantine");
  await settle(60);
  assertEquals(PAGE_POOL.stat.live_bytes, before);
});

Deno.test("release_terrain - a shared page is never released", async () => {
  const before = PAGE_POOL.stat.live_bytes;
  const ocean = into_core(decoded_page(3), false);
  release_terrain(ocean, 0);
  await settle(20);
  assert(PAGE_POOL.stat.live_bytes > before, "the ocean page stays borrowed");
});
