// @xpute/kit/abi/word.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { FIELD_MASK, FIELD_MASK64 } from "./word.ts";

// The expected values are the answers of xpute-kit's `field_mask32` and
// `field_mask64`.
test("a field mask is the field, the whole word, or nothing past the word", () => {
  assert.deepStrictEqual(
    [
      FIELD_MASK(3),
      FIELD_MASK(3, 5),
      FIELD_MASK(8, 24),
      FIELD_MASK(32),
      FIELD_MASK(3, 32),
    ],
    [0b111, 0b11100000, 0xff000000, 0xffffffff, 0],
  );
  assert.deepStrictEqual(
    [FIELD_MASK64(3, 5), FIELD_MASK64(64), FIELD_MASK64(3, 64)],
    [0b11100000n, 0xffff_ffff_ffff_ffffn, 0n],
  );
});
