// @xpute/host/abi/handle.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { GENERATION_BITS, SLOT_BITS } from "./handle.ts";

test("a handle's slot and generation fill its word", () => {
  assert.deepStrictEqual(SLOT_BITS + GENERATION_BITS, 32);
});
