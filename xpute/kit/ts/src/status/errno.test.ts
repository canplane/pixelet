// @xpute/kit/status/errno.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { Errno, errno_name, errno_of } from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/error.ts";

test("each errno is its own number and named", () => {
  const seen = new Map<number, string>();
  for (const [name, code] of Object.entries(Errno)) {
    assert.equal(
      seen.get(code),
      undefined,
      `${name} is ${seen.get(code)}'s number`,
    );
    seen.set(code, name);
    assert.equal(errno_of(code), code, name);
    assert.equal(errno_name(code), name);
    assert.ok(strerror(code).startsWith(`${name}:`), name);
  }
});
