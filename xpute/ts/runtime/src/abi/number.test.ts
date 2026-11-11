// @xpute/runtime/abi/number.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { MarshalError } from "@xpute/kit/status/error.ts";
import { FINITE, INT_I32, INT_U32, INT_U8, MEMBER } from "./number.ts";

const refused = (e: MarshalError) => e.errnum === Errno.EINVAL;

test("a number its type holds is read as it is, as xpute-core abi/number.rs reads it", () => {
  assert.deepStrictEqual(
    [FINITE(1.5), INT_U8(255), INT_U32(0xffff_ffff), INT_I32(-0x8000_0000)],
    [1.5, 255, 0xffff_ffff, -0x8000_0000],
  );
  assert.deepStrictEqual(MEMBER(7, { A: 7 }), 7);
  assert.deepStrictEqual(MEMBER(3, { A: 1, B: 2 }, 2), 2, "unknown");
});

test("a number its type does not hold is EINVAL", () => {
  for (const read of [
    () => FINITE(NaN),
    () => FINITE(Infinity),
    () => INT_U8(256),
    () => INT_U32(1.5),
    () => INT_U32(-1),
    () => INT_I32(0x8000_0000),
    () => MEMBER(3, { A: 1 }),
  ]) {
    assert.throws(read, refused);
  }
});
