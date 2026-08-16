// @pxl/shared/spec/space/pvf3/core.test.ts

import { assertEquals } from "@std/assert";

import { fvec, ivec } from "./mod.ts";

Deno.test("pvf3 core - ivec exposes the full signed 24/24/16 lane widths", () => {
  const packed = ivec.gather(-(1n << 23n), (1n << 23n) - 1n, -(1n << 15n));

  assertEquals(ivec.X(packed), -(1n << 23n));
  assertEquals(ivec.Y(packed), (1n << 23n) - 1n);
  assertEquals(ivec.Z(packed), -(1n << 15n));
});

Deno.test("pvf3 core - fvec exposes all 10 bits in every residue lane", () => {
  const max = (1n << 10n) - 1n;
  const packed = fvec.gather(max, max, max);

  assertEquals(fvec.X(packed), max);
  assertEquals(fvec.Y(packed), max);
  assertEquals(fvec.Z(packed), max);
  assertEquals(fvec.not(0n), (1n << 30n) - 1n);
});
