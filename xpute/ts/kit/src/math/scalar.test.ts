// @xpute/kit/math/scalar.test.ts

/** Holds the TypeScript side to golden/math/scalar.tsv, bit for bit. */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/kit/golden.ts";
import { approx_eq, ceil_to_step, clamp, floor_to_step, inv_lerp, lerp, near, remap, round_to_step, wrap } from "@xpute/kit/math/scalar.ts";

const bits = (x: number): bigint => {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  return view.getBigUint64(0);
};

Deno.test("scalar computes what the record holds", async () => {
  const v = await Golden.load("golden/math/scalar.tsv");
  v.each("", (k) => {
    const x = v.f64(`${k}.x`);
    const s = v.f64(`${k}.s`);
    const same = (name: string, got: number) => assertEquals(bits(got), bits(v.f64(`${k}.${name}`)), `${k}.${name}(${x}, ${s}) = ${got}`);
    assertEquals(approx_eq(x, x + s * 1e-6), v.bool(`${k}.approx_eq`), `${k}.approx_eq`);
    assertEquals(near(x, s, 0.3), v.bool(`${k}.near`), `${k}.near`);
    same("lerp", lerp(x, s, 0.3));
    same("inv_lerp", inv_lerp(x, -s, s * 2));
    same("remap", remap(x, -1, 1, 0, s));
    same("clamp", clamp(x, -s, s));
    same("wrap", wrap(x, -180, 180));
    same("floor_to_step", floor_to_step(x, s));
    same("round_to_step", round_to_step(x, s));
    same("ceil_to_step", ceil_to_step(x, s));
  });
});
