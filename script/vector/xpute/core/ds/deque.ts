// script/vector/xpute/core/ds/deque.ts
//
// A seeded sequence on a ring of fixed capacity, for
// crates/xpute/core/src/ds/deque.test.rs.

import { Deque } from "@xpute/core/ds/deque.ts";
import { seeded } from "../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0xdec);
  const d = new Deque<number>(new Array(8), 0, 0);
  const steps = [];
  for (let k = 0; k < 200; k++) {
    const r = rnd() % 4;
    let popped: number | string = "none";
    if (r === 0 && !d.full()) d.push_back(k);
    else if (r === 1 && !d.full()) d.push_front(k);
    else if (r === 2 && !d.empty()) popped = d.pop_front();
    else if (r === 3 && !d.empty()) popped = d.pop_back();
    steps.push({ r, popped, head: d.head, len: d.len, front: d.front() ?? "none", back: d.back() ?? "none" });
  }
  return steps;
}
