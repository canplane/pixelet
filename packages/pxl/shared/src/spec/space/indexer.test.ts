// @pxl/shared/spec/space/indexer.test.ts

import { assertEquals } from "@std/assert";

import { ivec, projection, type PVF3 } from "./pvf3/mod.ts";
import { FROM_L3_IDX, L2_BASE, L2_CENTER, L2_RANGE, L2_SPAN_IN_L3, L3_BASE, L3_CENTER, L3_IDX, L3_LIMIT, L3_RANGE, SPAN_CENTER } from "./indexer.ts";

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

/** The half has to track the span. A marker that snaps to a cell centre with
 * a fixed half is right at exactly one tier and wrong at every other, which
 * is what happens the moment the thing being marked changes LOD. */
Deno.test("indexer - SPAN_CENTER's half is half of the span, at every span", () => {
  const addr = projection.project({ i: 0n, f: 0n }, 100.7, 200.2, 33.9).i;
  const out: PVF3 = { i: 0n, f: 0n };

  for (const [shamt, span] of [[0, 1], [1, 2], [6, 64], [10, 1024]] as const) {
    const centre = projection.unproject([0, 0, 0], ...Object.values(SPAN_CENTER(out, addr, shamt)) as [bigint, bigint]);
    const floor = (v: number) => Math.floor(v / span) * span + span / 2;
    assertEquals(centre[0], floor(100), `x at span ${span}`);
    assertEquals(centre[1], floor(200), `y at span ${span}`);
    assertEquals(centre[2], floor(33), `z at span ${span}`);
  }
});

/** A 1u cell's centre is 0.5u, which is not an integer — so it lands in the
 * fractional lane, and the packed-OR the named CENTER helpers use would drop
 * it. This is the case that makes the parameterised form necessary. */
Deno.test("indexer - SPAN_CENTER at 1u puts the half in the fractional lane", () => {
  const out = SPAN_CENTER({ i: 0n, f: 0n }, ivec.gather(10n, -3n, 7n), 0);
  assertEquals(ivec.repr(out.i), "10:-3:7");
  assertEquals(out.f !== 0n, true);
  assertEquals(projection.unproject([0, 0, 0], out.i, out.f), [10.5, -2.5, 7.5]);
});

/** The stability the whole thing rests on: where a badge sits must depend on
 * the cell, never on which member of it was asked. Two points in one cell
 * have to resolve to the same anchor, or the badge moves when the camera
 * does. */
Deno.test("indexer - every point in a cell resolves to the same centre", () => {
  const a = SPAN_CENTER({ i: 0n, f: 0n }, projection.project({ i: 0n, f: 0n }, 100.1, 200.9, 33.2).i, 6);
  const b = SPAN_CENTER({ i: 0n, f: 0n }, projection.project({ i: 0n, f: 0n }, 120.8, 250.4, 60.7).i, 6);
  assertEquals(a.i, b.i);
  assertEquals(a.f, b.f);
  // and it agrees with the named helper at the level it names
  assertEquals(ivec.repr(a.i), ivec.repr(L2_CENTER(projection.project({ i: 0n, f: 0n }, 100.1, 200.9, 33.2).i)));
  assertEquals(a.f, 0n);
});
