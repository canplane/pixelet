// @xpute/runtime/mem/transfer.test.ts

import { assertEquals } from "@std/assert";

import { TransferPool } from "./transfer.ts";

Deno.test("slabs - the cap is the backpressure, and a slab that comes home is reused before a new one is made", () => {
  const slabs = new TransferPool(1 << 10, 2, 1000);
  const a = slabs.borrow(1n)!;
  slabs.borrow(2n)!;
  assertEquals(slabs.borrow(3n), null);

  slabs.give_back(a.id, a.buffer, 100);
  const c = slabs.borrow(3n)!;
  assertEquals(c.id, a.id);
  assertEquals(c.buffer, a.buffer);
});

Deno.test("slabs - a forgotten demand frees its slot, and an idle slab is released after the wait", () => {
  const slabs = new TransferPool(1 << 10, 1, 1000);
  const a = slabs.borrow(1n)!;
  assertEquals(slabs.borrow(2n), null);
  slabs.forget(1n);
  const b = slabs.borrow(2n)!;
  assertEquals(b.id, a.id);

  slabs.give_back(b.id, b.buffer, 5000);
  assertEquals(slabs.sweep(5500), 0);
  assertEquals(slabs.sweep(6500), 1);
});
