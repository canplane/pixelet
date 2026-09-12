// @xpute/core/ds/typed_alloc.test.ts

import { assert, assertEquals } from "@std/assert";

import { BuddyAllocator } from "./buddy.ts";
import { buddy_alloc, HEAP_ALLOC } from "./typed_alloc.ts";
import { Vector } from "./arena.ts";

function pool(): BuddyAllocator {
  return new BuddyAllocator({ min_block_bytes: 1 << 12, region_bytes: 1 << 20, max_idle_regions: 1, poison: false, bypass: false });
}

Deno.test("typed_alloc - the heap allocator is new Ctor(n)", () => {
  const view = HEAP_ALLOC.take(Float64Array, 8);
  assertEquals(view.length, 8);
  assertEquals([...view], [0, 0, 0, 0, 0, 0, 0, 0]);
});

Deno.test("typed_alloc - a pooled view comes back zeroed even when its block was written", () => {
  const alloc = buddy_alloc(pool());
  const first = alloc.take(Int32Array, 1024);
  first.fill(-7);
  alloc.give(first);
  const again = alloc.take(Int32Array, 1024);
  assert(again.every((v) => v === 0));
});

Deno.test("typed_alloc - BigInt lanes are zeroed too", () => {
  const alloc = buddy_alloc(pool());
  const view = alloc.take(BigInt64Array, 16);
  assertEquals(view[15], 0n);
});

Deno.test("typed_alloc - giving a view back returns its block to the pool", () => {
  const p = pool();
  const alloc = buddy_alloc(p);
  const before = p.stat.live_bytes;
  const view = alloc.take(Float32Array, 4096);
  assert(p.stat.live_bytes > before);
  alloc.give(view);
  assertEquals(p.stat.live_bytes, before);
});

Deno.test("Vector - growing from a pool hands the old storage back", () => {
  const p = pool();
  const v = new Vector(Uint32Array, { init_cap: 1024, alloc: buddy_alloc(p) });
  for (let i = 0; i < 5000; i++) v.push(i);
  assertEquals(v.get(4999), 4999);
  assertEquals(v.get(0), 0);
  assertEquals(p.stat.live_bytes, v.cap * 4, "only the current storage is borrowed");
});
