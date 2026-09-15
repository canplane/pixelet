// @xpute/core/sys/slab.test.ts

import { assertEquals } from "@std/assert";

import { fnv1a64_str } from "../kit/hash.ts";
import { LittleSlabs } from "./slab.ts";

Deno.test("slabs - the cap is the backpressure, and a slab that comes home is reused before a new one is made", () => {
  const slabs = new LittleSlabs(1 << 10, 2, 1000);
  const a = slabs.borrow("job:1")!;
  const b = slabs.borrow("job:2")!;
  assertEquals(slabs.borrow("job:3"), null);
  assertEquals(slabs.stats(), { cap: 2, home: 0, away: 2, bytes: 2048 });

  slabs.give_back(a.id, a.buffer, 100);
  const c = slabs.borrow("job:3")!;
  assertEquals(c.id, a.id);
  assertEquals(c.buffer, a.buffer);
  assertEquals(slabs.owner_of(c.id), fnv1a64_str("job:3"));
  assertEquals(slabs.owner_of(b.id), fnv1a64_str("job:2"));
});

Deno.test("slabs - a forgotten job frees its slot, and an idle slab is released after the wait", () => {
  const slabs = new LittleSlabs(1 << 10, 1, 1000);
  const a = slabs.borrow("job:1")!;
  assertEquals(slabs.borrow("job:2"), null);
  slabs.forget("job:1");
  assertEquals(slabs.owner_of(a.id), null);
  const b = slabs.borrow("job:2")!;
  assertEquals(b.id, a.id);

  slabs.give_back(b.id, b.buffer, 5000);
  assertEquals(slabs.sweep(5500), 0);
  assertEquals(slabs.stats().home, 1);
  assertEquals(slabs.sweep(6500), 1);
  assertEquals(slabs.stats(), { cap: 1, home: 0, away: 0, bytes: 0 });
});

Deno.test("slabs - giving back a slab that is not away changes nothing", () => {
  const slabs = new LittleSlabs(1 << 10, 1, 1000);
  const a = slabs.borrow("job:1")!;
  slabs.give_back(a.id, a.buffer, 1);
  slabs.give_back(a.id, a.buffer, 2);
  assertEquals(slabs.stats().home, 1);
});
