// script/vector/xpute/core/kit/comp.ts
//
// comp_str over pairs that UTF-16 and code point order disagree on, for
// crates/xpute/core/src/kit/comp.test.rs.

import { comp_numeric, comp_str } from "@xpute/core/kit/comp.ts";

export function vectors() {
  const pairs = [["a", "b"], ["b", "a"], ["", "a"], ["abc", "abc"], ["｡", "\u{1F600}"], ["\u{1F600}", "￿"], ["", "\u{10000}"], ["한", "韓"]];
  return {
    str: pairs.map(([a, b]) => ({ a, b, c: comp_str(a, b) })),
    numeric: [[1, 2], [2, 1], [0, -0], [NaN, 1], [1n, 2n]].map(([a, b]) => comp_numeric(a as number, b as number)),
  };
}
