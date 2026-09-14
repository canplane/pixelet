// script/vector/xpute/core/ds/treap_set.ts
//
// A seeded treap under inserts and deletes, and its order statistics and
// windows, for crates/xpute/core/src/ds/treap_set.test.rs.

import { TreapSet } from "@xpute/core/ds/treap_set.ts";
import { comp_numeric } from "@xpute/core/kit/comp.ts";
import { seeded } from "../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0x7ea9);
  const t = new TreapSet<number>(comp_numeric, 42n);
  const ops = [];
  for (let k = 0; k < 400; k++) {
    const v = rnd() % 150;
    const insert = rnd() % 4 !== 0;
    if (insert) t.insert(v);
    else t.delete(v);
    ops.push({ insert, v, size: t.size() });
  }
  return {
    ops,
    kth: [0, 1, 5, t.size() - 1, t.size()].map((k) => t.kth(k) ?? "none"),
    iter: [...t.iter()],
    prefix: [...t.iter_prefix(7)],
    from: [...t.iter_from(140, 12)],
    from_mid: [...t.iter_from(33, 5)],
    has: [0, 33, 149, 150].map((v) => t.has(v)),
  };
}
