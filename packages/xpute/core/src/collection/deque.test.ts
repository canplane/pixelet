// @xpute/core/collection/deque.test.ts

/** Holds the TypeScript side to golden/collection/deque.tsv, which every language's ring must match. */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/core/golden.ts";
import { Deque } from "@xpute/core/collection/deque.ts";

const show = (x: number | undefined): string => x === undefined ? "none" : `${x}`;

Deno.test("deque holds what the record holds", async () => {
  const v = await Golden.load("golden/collection/deque.tsv");
  const d = new Deque<number>(new Array(8).fill(undefined));
  v.each("", (k) => {
    const step = Number(k);
    const r = v.u32(`${k}.r`);
    let popped = "none";
    if (r === 0 && !d.full()) d.push_back(step);
    else if (r === 1 && !d.full()) d.push_front(step);
    else if (r === 2) popped = show(d.pop_front());
    else if (r === 3) popped = show(d.pop_back());
    assertEquals(popped, v.s(`${k}.popped`), `${k}.popped`);
    assertEquals([d.head, d.len], [v.u32(`${k}.head`), v.u32(`${k}.len`)], k);
    assertEquals(show(d.front()), v.s(`${k}.front`), `${k}.front`);
    assertEquals(show(d.back()), v.s(`${k}.back`), `${k}.back`);
  });
});
