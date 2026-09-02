// @pxl/shared/spec/space/indexer.test.ts

import { assertEquals } from "@std/assert";

import { ivec } from "./pvf3/mod.ts";
import { FROM_L3_IDX, L2_BASE, L2_RANGE, L2_SPAN_IN_L3, L3_BASE, L3_CENTER, L3_IDX, L3_LIMIT, L3_RANGE } from "./indexer.ts";

Deno.test("indexer - L2 is a signed 64-cubed render chunk", () => {
  const addr = ivec.gather(-1n, 95n, -65n);
  assertEquals(L2_RANGE, 64n);
  assertEquals(ivec.repr(L2_BASE(addr)), "-64:64:-128");
});

Deno.test("indexer - L3 is a signed 1024-cubed addressing level, same shape as L2", () => {
  // Z=1801 (not just <1024, like L2's own test addr) so a nonzero Z
  // BASE/CENTER/LIMIT actually proves Z participates, instead of a Z<1024
  // value that floors to 0 either way and would silently pass even with the
  // old XY-only behavior.
  const addr = ivec.gather(-1n, 2049n, 1801n);
  assertEquals(L3_RANGE, 1024n);
  assertEquals(L2_SPAN_IN_L3, 16n);
  // Z participates like X/Y — no XY-only narrowing at this level (that's
  // terrain_page.ts's own policy, tested separately).
  assertEquals(ivec.repr(L3_BASE(addr)), "-1024:2048:1024");
  assertEquals(ivec.repr(L3_CENTER(addr)), "-512:2560:1536");
  assertEquals(ivec.repr(L3_LIMIT(addr)), "0:3072:2048");
});

Deno.test("indexer - L3_IDX/FROM_L3_IDX round-trip on all three axes", () => {
  const addr = ivec.gather(-1n, 2049n, 1801n);
  const idx = L3_IDX(addr);
  assertEquals(ivec.repr(idx), "-1:2:1");
  assertEquals(ivec.repr(FROM_L3_IDX(idx)), "-1024:2048:1024");
});
