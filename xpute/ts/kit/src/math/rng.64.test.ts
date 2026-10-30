// @xpute/kit/math/rng.64.test.ts

/**
 * Holds the TypeScript side to golden/math/rng.64.tsv, which every language's
 * stream must match. Its draws were computed apart from the three, in Python's
 * exact integers masked to 64 bits, whose stream matches rand_xoshiro's
 * SplitMix64 reference vector.
 */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/kit/golden.ts";
import {
  derive_seed_u64,
  make_rng,
  mix64,
  u32_to_unit,
} from "@xpute/kit/math/rng.ts";

test("rng computes what the record holds", async () => {
  const v = await Golden.load("golden/math/rng.64.tsv");
  v.each("", (k) => {
    const seed = v.u64(`${k}.seed`);
    assert.deepStrictEqual(mix64(seed), v.u64(`${k}.mix64`), `${k}.mix64`);
    assert.deepStrictEqual(
      derive_seed_u64(seed, 77n),
      v.u64(`${k}.derive`),
      `${k}.derive`,
    );
    const draw = make_rng(seed);
    v.each(`${k}.draws`, (d) => {
      const got = draw();
      assert.deepStrictEqual(got, v.u32(d), d);
      const j = d.slice(d.lastIndexOf(".") + 1);
      assert.deepStrictEqual(
        Object.is(u32_to_unit(got), v.f64(`${k}.unit.${j}`)),
        true,
        `${k}.unit.${j}`,
      );
    });
  });
});
