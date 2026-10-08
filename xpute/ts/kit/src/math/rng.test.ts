// @xpute/kit/math/rng.test.ts

/** Holds the TypeScript side to golden/math/rng.tsv, which every language's stream must match. */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/kit/golden.ts";
import { derive_seed_u64, make_rng, mix64, u32_to_unit } from "@xpute/kit/math/rng.ts";

Deno.test("rng computes what the record holds", async () => {
  const v = await Golden.load("golden/math/rng.tsv");
  v.each("", (k) => {
    const seed = v.u64(`${k}.seed`);
    assertEquals(mix64(seed), v.u64(`${k}.mix64`), `${k}.mix64`);
    assertEquals(derive_seed_u64(seed, 77n), v.u64(`${k}.derive`), `${k}.derive`);
    const draw = make_rng(seed);
    v.each(`${k}.draws`, (d) => {
      const got = draw();
      assertEquals(got, v.u32(d), d);
      const j = d.slice(d.lastIndexOf(".") + 1);
      assertEquals(Object.is(u32_to_unit(got), v.f64(`${k}.unit.${j}`)), true, `${k}.unit.${j}`);
    });
  });
});
