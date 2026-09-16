// @xpute/core/ds/typed_alloc.test.ts

import { assertEquals } from "@std/assert";

import { HEAP_ALLOC } from "./typed_alloc.ts";

Deno.test("typed_alloc - the heap allocator is new Ctor(n)", () => {
  const view = HEAP_ALLOC.take(Float64Array, 8);
  assertEquals(view.length, 8);
  assertEquals([...view], [0, 0, 0, 0, 0, 0, 0, 0]);
});
