// @xpute/kit/math/rng.test.ts

/**
 * Holds the TypeScript streams to golden/math/rng.32.tsv and rng.64.tsv, the
 * records every language's stream must match. The 64-bit record's draws come
 * from none of the three implementations: they are computed in Python's exact
 * integers masked to 64 bits, and that stream matches rand_xoshiro's
 * SplitMix64 reference vector.
 */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import {
  make_rng,
  make_rng32,
  mix32,
  mix64,
  seed_mix32,
  seed_mix64,
  u32_to_unit,
} from "@xpute/kit/math/rng.ts";

// ---- the 32-bit family ----

test("rng32 computes what the record holds", async () => {
  const v = await Golden.load("kit/spec/golden/math/rng.32.tsv");
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

// ---- the 64-bit family ----

test("rng computes what the record holds", async () => {
  const v = await Golden.load("kit/spec/golden/math/rng.64.tsv");
  v.each("", (k) => {
    const seed = v.u64(`${k}.seed`);
    assert.deepStrictEqual(mix64(seed), v.u64(`${k}.mix64`), `${k}.mix64`);
    assert.deepStrictEqual(
      seed_mix64(seed, 77n),
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
