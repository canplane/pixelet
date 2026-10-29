// @xpute/host/abi/handle.test.ts

import { test } from "vitest";
import { assertEquals } from "@std/assert";

import { GENERATION_BITS, SLOT_BITS } from "./handle.ts";

test("a handle's slot and generation fill its word", () => {
  assertEquals(SLOT_BITS + GENERATION_BITS, 32);
});
