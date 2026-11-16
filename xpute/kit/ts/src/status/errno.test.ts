// @xpute/kit/status/errno.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { name_Errno, of_Errno } from "@xpute/kit/status/errno.spec.ts";
import { strerror } from "@xpute/kit/status/error.ts";

test("each errno is named once, and read by its name", () => {
  const seen = new Set<string>();
  for (let n = -255; n <= 0; n++) {
    const e = of_Errno(n);
    if (e === null) continue;
    const name = name_Errno(e);
    assert.ok(!seen.has(name), `${name} names two numbers`);
    seen.add(name);
    assert.ok(strerror(n).startsWith(`${name}:`), name);
  }
});
