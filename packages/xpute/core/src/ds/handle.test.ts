// @xpute/core/ds/handle.test.ts

import { assert, assertEquals, assertNotEquals } from "@std/assert";

import { handle_slot, HandleTable } from "./handle.ts";

Deno.test("handle - a released handle no longer names its slot, and the slot's next handle differs", () => {
  const t = new HandleTable(4);
  const a = t.acquire()!;
  assert(t.live(a));
  assert(t.release(a));
  assert(!t.live(a));
  assert(!t.release(a), "a handle is released once");
  const b = t.acquire()!;
  assertEquals(handle_slot(b), handle_slot(a), "the slot is reused");
  assertNotEquals(b, a);
  assert(t.live(b) && !t.live(a));
});

Deno.test("handle - 0 is never a handle, and a full table refuses", () => {
  const t = new HandleTable(3);
  assert(!t.live(0));
  const handles = [t.acquire()!, t.acquire()!];
  assert(handles.every((h) => h !== 0 && handle_slot(h) !== 0));
  assertEquals(t.acquire(), null);
  assertEquals(t.count(), 2);
  t.release(handles[0]);
  assert(t.acquire() !== null);
});
