// @xpute/kit/status/errno.test.ts

/**
 * Checks the errno constants: no two share a number, and `strerror` names
 * each one.
 */

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";

test("each errno is its own number and named", () => {
  const seen = new Map<number, string>();
  for (const [name, code] of Object.entries(Errno)) {
    if (typeof code !== "number") continue;
    assert.equal(
      seen.get(code),
      undefined,
      `${name} is ${seen.get(code)}'s number`,
    );
    seen.set(code, name);
    assert.equal(Errno.errno_of(code), code, name);
    assert.ok(Errno.strerror(code).startsWith(`${name}:`), name);
  }
});
