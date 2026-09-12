// @xpute/core/ds/pool.test.ts

import { assert, assertEquals } from "@std/assert";

import { BuddyAllocator } from "./buddy.ts";

/** 4KB blocks in a 64KB region: 16 leaves, 5 orders. Small enough to reason
 * about exactly, which is the point — buddy's failure modes are arithmetic. */
function pool(overrides: Record<string, unknown> = {}) {
  return new BuddyAllocator({ min_block_bytes: 1 << 12, region_bytes: 1 << 16, ...overrides });
}

Deno.test("pool - a released block's exact span is handed to the next request of that order", () => {
  const p = pool();
  const first = p.acquire(1 << 12);
  const span = first.byte_offset;
  p.release(first);

  const second = p.acquire(1 << 12);
  assertEquals(second.byte_offset, span);
  assertEquals(p.stat.regions, 1);
});

Deno.test("pool - blocks never overlap while they are all live", () => {
  const p = pool();
  const live = Array.from({ length: 16 }, () => p.acquire(1 << 12));
  const ranges = live.map((b) => [b.byte_offset, b.byte_offset + (1 << 12)] as const).sort((a, b) => a[0] - b[0]);
  for (let i = 1; i < ranges.length; i++) assert(ranges[i][0] >= ranges[i - 1][1], `overlap at ${i}`);
  assertEquals(p.stat.regions, 1); // exactly filled it
  assertEquals(p.stat.live_blocks, 16);
});

/**
 * The whole reason this is a buddy allocator rather than per-class free
 * lists: a workload that frees one order and immediately asks for another
 * (a LOD tier change) must be able to move bytes between orders in both
 * directions. Splitting covers the shrink, merging covers the grow back.
 */
Deno.test("pool - a freed large block splits to serve smaller ones, and merges back", () => {
  const p = pool();
  const big = p.acquire(1 << 16); // the whole region, one block
  assertEquals(p.stat.regions, 1);
  p.release(big);

  const small = Array.from({ length: 16 }, () => p.acquire(1 << 12));
  assertEquals(p.stat.regions, 1); // split out of what was freed, never grown
  assert(p.stat.splits > 0);

  for (const b of small) p.release(b);
  assert(p.stat.merges > 0);

  // ...and the merged space serves the original size again.
  const again = p.acquire(1 << 16);
  assertEquals(p.stat.regions, 1);
  assertEquals(again.byte_offset, 0);
  p.release(again);
});

Deno.test("pool - a request larger than a region gets its own buffer and is never pooled", () => {
  const p = pool();
  const huge = p.acquire((1 << 16) + 1);
  assertEquals(p.stat.spills, 1);
  assertEquals(p.stat.regions, 0);
  assertEquals(huge.byte_offset, 0);
  p.release(huge);
  assertEquals(p.stat.regions, 0);
});

Deno.test("pool - empty regions past the idle ceiling go back to the collector", () => {
  const p = pool({ max_idle_regions: 1 });
  const blocks = Array.from({ length: 48 }, () => p.acquire(1 << 12)); // 3 regions
  assertEquals(p.stat.regions, 3);
  for (const b of blocks) p.release(b);
  assertEquals(p.stat.regions, 1);
  assertEquals(p.stat.evictions, 2);
});

Deno.test("pool - a double release is refused rather than corrupting the free lists", () => {
  const p = pool();
  const b = p.acquire(1 << 12);
  p.release(b);
  p.release(b);
  assertEquals(p.stat.live_blocks, 0);
  // The span is handed out once, not twice.
  const x = p.acquire(1 << 12);
  const y = p.acquire(1 << 12);
  assert(x.byte_offset !== y.byte_offset);
});

Deno.test("pool - bypass hands every request its own buffer and reuses nothing", () => {
  const p = pool({ bypass: true });
  const first = p.acquire(1 << 12);
  p.release(first);
  const second = p.acquire(1 << 12);
  assert(first.buf !== second.buf);
  assertEquals(p.stat.regions, 0);
  assertEquals(p.stat.spills, 2);
});

// ---- backing ----

const REGION = 1 << 16;

function backed(regions: number, overrides: Record<string, unknown> = {}) {
  const backing = new ArrayBuffer(REGION * regions);
  return { backing, pool: pool({ backing, max_idle_regions: 0, ...overrides }) };
}

Deno.test("pool - regions come out of the backing first, at absolute offsets", () => {
  const { backing, pool: p } = backed(2);
  const a = p.acquire(REGION);
  const b = p.acquire(REGION);
  assert(a.buf === backing && b.buf === backing, "both regions carved from the backing");
  assertEquals([a.byte_offset, b.byte_offset].sort((x, y) => x - y), [0, REGION]);

  // Writing through one block must not show up in the other.
  new Uint8Array(a.buf, a.byte_offset, a.bytes).fill(1);
  new Uint8Array(b.buf, b.byte_offset, b.bytes).fill(2);
  assertEquals(new Uint8Array(a.buf, a.byte_offset, a.bytes).every((v) => v === 1), true);
});

Deno.test("pool - a block in a backed region past the first round-trips through release", () => {
  // The second region's base is not zero, so release has to subtract it
  // before the buddy arithmetic, or it frees the wrong span.
  const { pool: p } = backed(2);
  const first = p.acquire(REGION);
  const small = p.acquire(1 << 12);
  assertEquals(small.byte_offset >= REGION, true);
  p.release(small);
  const again = p.acquire(1 << 12);
  assertEquals(again.byte_offset, small.byte_offset);
  p.release(again);
  p.release(first);
});

Deno.test("pool - past the backing it falls back to plain regions, and only those are reclaimed", () => {
  const { backing, pool: p } = backed(1);
  const a = p.acquire(REGION);
  const b = p.acquire(REGION);
  assert(a.buf === backing, "first region is the backing");
  assert(b.buf !== backing, "second region is a plain buffer");
  assertEquals(p.stat.regions, 2);

  p.release(a);
  p.release(b);
  // max_idle_regions is 0: the plain region goes, the backed one stays.
  assertEquals(p.stat.regions, 1);
  p.clear();
  assertEquals(p.stat.regions, 1);

  // And the backed region is what serves the next request.
  const c = p.acquire(1 << 12);
  assert(c.buf === backing, "reused from the backing");
});

Deno.test("pool - a backing smaller than a region is not used at all", () => {
  const backing = new ArrayBuffer(REGION - 1);
  const p = pool({ backing });
  assert(p.acquire(1 << 12).buf !== backing);
});
