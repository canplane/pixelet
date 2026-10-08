// @xpute/kit/collection/heap.test.ts

/** Holds the TypeScript side to golden/collection/heap.tsv, which every language's heaps must match. */

import { assertEquals } from "@std/assert";

import { Golden } from "@xpute/kit/golden.ts";
import { type Element, heapsort, MaxHeap, MinHeap } from "@xpute/kit/collection/heap.ts";

const show = (e: Element<number, number> | undefined): string => e === undefined ? "none" : `${e.key}:${e.val}`;

Deno.test("heaps pop what the record holds", async () => {
  const v = await Golden.load("golden/collection/heap.tsv");
  const min = new MinHeap<number, number>([]);
  const max = new MaxHeap<number, number>([]);
  v.each("steps", (k) => {
    if (v.s(`${k}.op`) === "pop") {
      assertEquals(show(min.pop()), v.s(`${k}.min`), `${k}.min`);
      assertEquals(show(max.pop()), v.s(`${k}.max`), `${k}.max`);
    } else {
      const e = { key: Number(v.s(`${k}.key`)), val: Number(v.s(`${k}.val`)) };
      min.push(e);
      max.push(e);
    }
  });
  const arr: Element<number, number>[] = [];
  v.each("input", (k) => {
    const [key, val] = v.s(k).split(":").map(Number);
    arr.push({ key, val });
  });
  heapsort(arr);
  const want: string[] = [];
  v.each("sorted", (k) => want.push(v.s(k)));
  assertEquals(arr.map(show), want);
});
