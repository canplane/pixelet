// @xpute/kit/math/rng.32.test.ts

/** Holds the TypeScript side to golden/math/rng.32.tsv, which every language's stream must match. */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/kit/golden.ts";
import { derive_seed_u32, make_rng32, mix32, u32_to_unit } from "@xpute/kit/math/rng.ts";

Deno.test("rng32 computes what the record holds", async () => {
  const v = await Golden.load("golden/math/rng.32.tsv");
  v.each("", (k) => {
    const seed = v.u32(`${k}.seed`);
    assertEquals(mix32(seed), v.u32(`${k}.mix32`), `${k}.mix32`);
    assertEquals(derive_seed_u32(seed, 77), v.u32(`${k}.derive`), `${k}.derive`);
    const draw = make_rng32(seed);
    v.each(`${k}.draws`, (d) => {
      const got = draw();
      assertEquals(got, v.u32(d), d);
      const j = d.slice(d.lastIndexOf(".") + 1);
      assertEquals(Object.is(u32_to_unit(got), v.f64(`${k}.unit.${j}`)), true, `${k}.unit.${j}`);
    });
  });
});
