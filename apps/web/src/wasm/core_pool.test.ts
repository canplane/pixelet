// @pxl/web/wasm/core_pool.test.ts

import { assert, assertEquals } from "@std/assert";

import type { u32 } from "@xpute/core/kit/word.ts";
import { type Block, BuddyAllocator } from "@xpute/core/ds/buddy.ts";

import { CORE_MEMORY, CORE_PXL, CORE_SECTIONS, FIXED_ALLOC, MODULE_SECTION } from "@/wasm/core_pool.ts";

function rng(seed: u32): () => u32 {
  let s = seed >>> 0 || 1;
  return () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return s >>> 0;
  };
}

Deno.test("core pool - the module is over the memory and every section's blocks come from it", () => {
  assert(CORE_MEMORY !== null && CORE_PXL !== null);
  for (const name of ["fixed", "page", "mesh"] as const) {
    const s = CORE_SECTIONS[name];
    const block = s.pool.acquire(1 << 12);
    assert(block.buf === CORE_MEMORY.buffer, `${name}: in the core memory`);
    assert(block.byte_offset >= s.offset && block.byte_offset < s.offset + s.bytes, `${name}: inside its section`);
    assert(block.byte_offset >= MODULE_SECTION.bytes, `${name}: past the module's section`);
    assertEquals(s.pool.stat.live_blocks >= 1, true);
    s.pool.release(block);
  }
});

Deno.test("core pool - the module's buddy hands out the same addresses the TypeScript buddy does for the same requests", () => {
  assert(CORE_MEMORY !== null && CORE_PXL !== null);
  const REGION = 1 << 16;
  const RANGE = 1 << 22;
  // A range of the fixed section for a pool of the module's, against a
  // TypeScript pool over a buffer of the same shape.
  const range = FIXED_ALLOC.take(Uint8Array, RANGE);
  const base = range.byteOffset;
  assertEquals(base % REGION, 0, "a 4 MB buddy block is region-aligned");
  const id = CORE_PXL.pxl_pool_new(base, RANGE, REGION, 1 << 12);
  const ts = new BuddyAllocator({ backing: new ArrayBuffer(RANGE), region_bytes: REGION, min_block_bytes: 1 << 12, no_spill: true, max_idle_regions: 0 });

  const next = rng(41);
  const live: { at: u32; block: Block; bytes: u32 }[] = [];
  let acquired = 0, refused = 0;
  for (let i = 0; i < 4000; i++) {
    if (live.length > 0 && next() % 3 === 0) {
      const k = next() % live.length;
      const [l] = live.splice(k, 1);
      assertEquals(CORE_PXL.pxl_pool_release(id, l.at, l.bytes), 1);
      ts.release(l.block);
      continue;
    }
    const bytes = (1 << (12 + (next() % 6))) - (next() % 100);
    const at = CORE_PXL.pxl_pool_acquire(id, bytes);
    const block = ts.try_acquire(bytes);
    if (at === 0) {
      assertEquals(block, null, `request ${i}: both refuse`);
      refused++;
      continue;
    }
    assert(block !== null, `request ${i}: both serve`);
    assertEquals(at - base, block.byte_offset, `request ${i}: same offset`);
    live.push({ at, block, bytes });
    acquired++;
  }
  assert(acquired > 1000 && refused > 0, `acquired ${acquired}, refused ${refused}`);
  const stat = new Uint32Array(CORE_MEMORY.buffer, CORE_PXL.pxl_pool_stat(id), 12);
  const s = ts.stat;
  assertEquals(Array.from(stat), [s.live_bytes, s.idle_bytes, s.reserved_bytes, s.regions, s.live_blocks, s.hits, s.misses, s.splits, s.merges, s.max_block_bytes, s.backing_used, s.refusals]);
  for (const l of live) CORE_PXL.pxl_pool_release(id, l.at, l.bytes);
  assertEquals(CORE_PXL.pxl_pool_release(id, live[0].at, live[0].bytes), 0, "a double release is refused");
  FIXED_ALLOC.give(range);
});
