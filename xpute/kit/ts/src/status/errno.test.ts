// @xpute/kit/status/errno.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { ERRNO } from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/error.ts";

test("each errno is its own number and named", () => {
  const seen = new Map<number, string>();
  for (const [name, { number }] of Object.entries(ERRNO)) {
    assert.equal(
      seen.get(number),
      undefined,
      `${name} is ${seen.get(number)}'s number`,
    );
    seen.set(number, name);
    assert.ok(strerror(number).startsWith(`${name}:`), name);
  }
});
