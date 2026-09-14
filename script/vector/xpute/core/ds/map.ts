// script/vector/xpute/core/ds/map.ts
//
// A seeded sequence of inserts and removals on both key widths, the
// positions and moves each reports, and the seeded walks, for
// crates/xpute/core/src/ds/map.test.rs.

import { I64_KEYS, IndexedMap, type KeyTraits, U32_KEYS } from "@xpute/core/ds/map.ts";
import { seeded } from "../../../../vector.ts";

function run<K extends number | bigint>(traits: KeyTraits<K>, make: (n: number) => K) {
  const rnd = seeded(0x3a9);
  const m = new IndexedMap<K>(traits, 2);
  const steps = [];
  for (let k = 0; k < 600; k++) {
    if (m.size > 0 && rnd() % 3 === 0) {
      const pos = rnd() % m.size;
      const key = m.key_at(pos);
      steps.push({ op: "remove", pos, key, from: m.remove_at(pos), size: m.size });
    } else {
      const key = make(rnd() % 2048);
      steps.push({ op: "insert", key, pos: m.insert(key), size: m.size });
    }
  }
  return { steps, walk: [...m.positions_seeded(99n)], walk2: [...m.positions_seeded(0xffffffffffffn)] };
}

export function vectors() {
  return {
    i64: run(I64_KEYS, (n) => (BigInt(n) << 40n) - 7n),
    u32: run(U32_KEYS, (n) => Math.imul(n, 2654435761) >>> 0),
  };
}
