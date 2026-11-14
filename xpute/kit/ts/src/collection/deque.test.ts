// @xpute/kit/collection/deque.test.ts

/**
 * Holds the TypeScript deque to golden/collection/deque.tsv, the record every
 * language's deque must match.
 */

import { test } from "vitest";
import assert from "node:assert/strict";

import { Golden } from "@xpute/golden";
import { Deque } from "@xpute/kit/collection/deque.ts";

const show = (x: number | undefined): string =>
  x === undefined ? "none" : `${x}`;

test("deque holds what the record holds", async () => {
  const v = await Golden.load("kit/spec/golden/collection/deque.tsv");
  const d = new Deque<number>(Array.from({ length: 8 }));
  v.each("", (k) => {
    const step = Number(k);
    const r = v.u32(`${k}.r`);
    let popped = "none";
    if (r === 0 && !d.is_full()) d.push_back(step);
    else if (r === 1 && !d.is_full()) d.push_front(step);
    else if (r === 2) popped = show(d.pop_front());
    else if (r === 3) popped = show(d.pop_back());
    assert.deepStrictEqual(popped, v.s(`${k}.popped`), `${k}.popped`);
    assert.deepStrictEqual(
      [d.head, d.len],
      [v.u32(`${k}.head`), v.u32(`${k}.len`)],
      k,
    );
    assert.deepStrictEqual(show(d.front()), v.s(`${k}.front`), `${k}.front`);
    assert.deepStrictEqual(show(d.back()), v.s(`${k}.back`), `${k}.back`);
  });
});
