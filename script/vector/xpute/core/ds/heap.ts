// script/vector/xpute/core/ds/heap.ts
//
// Min and max heaps under a seeded push/pop sequence with ties, and a
// heapsort, for crates/xpute/core/src/ds/heap.test.rs. The ties are the
// point: which of two equal keys comes out first is the algorithm's.

import { heapsort, MaxHeap, MinHeap } from "@xpute/core/ds/heap.ts";
import { seeded } from "../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0x4ea9);
  const min = new MinHeap<number>([]);
  const max = new MaxHeap<number>([]);
  const steps = [];
  for (let k = 0; k < 300; k++) {
    if (rnd() % 3 === 0) {
      const a = min.pop(), b = max.pop();
      steps.push({ op: "pop", min: a ? `${a.key}:${a.val}` : "none", max: b ? `${b.key}:${b.val}` : "none" });
    } else {
      const key = rnd() % 20;
      min.push({ key, val: k });
      max.push({ key, val: k });
      steps.push({ op: "push", key, val: k });
    }
  }
  const arr = Array.from({ length: 50 }, (_, i) => ({ key: rnd() % 10, val: i }));
  const input = arr.map((e) => `${e.key}:${e.val}`);
  heapsort(arr);
  return { steps, input, sorted: arr.map((e) => `${e.key}:${e.val}`) };
}
