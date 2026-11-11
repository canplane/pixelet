// @xpute/kit/math/rng.32.test.ts

/**
 * Holds the TypeScript stream to golden/math/rng.32.tsv, the record every
 * language's stream must match.
 */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "../../../dev/golden.ts";
import {
  seed_mix32,
  make_rng32,
  mix32,
  u32_to_unit,
} from "@xpute/kit/math/rng.ts";

test("rng32 computes what the record holds", async () => {
  const v = await Golden.load("golden/math/rng.32.tsv");
  v.each("", (k) => {
    const seed = v.u32(`${k}.seed`);
    assert.deepStrictEqual(mix32(seed), v.u32(`${k}.mix32`), `${k}.mix32`);
    assert.deepStrictEqual(
      seed_mix32(seed, 77),
      v.u32(`${k}.derive`),
      `${k}.derive`,
    );
    const draw = make_rng32(seed);
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
