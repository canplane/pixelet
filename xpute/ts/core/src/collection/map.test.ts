// @xpute/core/collection/map.test.ts

import { assertEquals } from "@std/assert";

import { I64_KEYS, IndexedMap, type KeyTraits, U32_KEYS } from "./map.ts";

Deno.test("IndexedMap - positions are dense and stable until a removal", () => {
  const m = new IndexedMap(I64_KEYS, 4);
  assertEquals(m.insert(10n), 0);
  assertEquals(m.insert(20n), 1);
  assertEquals(m.insert(10n), 0, "an existing key keeps its position");
  assertEquals(m.find(20n), 1);
  assertEquals(m.find(30n), -1);
});

Deno.test("IndexedMap - removal moves the last entry into the hole and says where it came from", () => {
  const m = new IndexedMap(U32_KEYS, 4);
  for (const k of [1, 2, 3]) m.insert(k);
  assertEquals(m.remove_at(0), 2);
  assertEquals(m.size, 2);
  assertEquals(m.key_at(0), 3);
  assertEquals(m.find(3), 0);
  assertEquals(m.find(1), -1);
  assertEquals(m.remove_at(1), 1, "removing the last moves nothing");
});

function agrees_with_map<K extends number | bigint>(traits: KeyTraits<K>, make: (n: number) => K): void {
  const m = new IndexedMap(traits, 2);
  const ref = new Map<K, number>();
  const live: K[] = [];
  let seed = 12345;
  const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff);
  for (let n = 0; n < 5000; n++) {
    if (live.length > 0 && rnd() % 3 === 0) {
      const k = live.splice(rnd() % live.length, 1)[0];
      const pos = m.find(k);
      const from = m.remove_at(pos);
      const moved = [...ref.entries()].find(([, p]) => p === from)![0];
      ref.delete(k);
      if (from !== pos) ref.set(moved, pos);
    } else {
      // Clustered keys, the case linear probing is worst at.
      const k = make(rnd() % 4096);
      if (!ref.has(k)) {
        live.push(k);
        ref.set(k, m.insert(k));
      }
    }
  }
  assertEquals(m.size, ref.size);
  for (const [k, pos] of ref) assertEquals(m.find(k), pos);
}

Deno.test("IndexedMap - agrees with a Map across growth and many removals, 64-bit keys", () => {
  agrees_with_map(I64_KEYS, (n) => BigInt(n) << 6n);
});

Deno.test("IndexedMap - agrees with a Map across growth and many removals, 32-bit keys", () => {
  agrees_with_map(U32_KEYS, (n) => n << 6);
});

Deno.test("IndexedMap - a 64-bit key past 2^63 finds itself", () => {
  const m = new IndexedMap(I64_KEYS);
  const big = (1n << 63n) + 5n;
  const pos = m.insert(big);
  assertEquals(m.find(big), pos);
});

Deno.test("IndexedMap - a seeded walk visits every position once", () => {
  const m = new IndexedMap(U32_KEYS);
  for (let k = 0; k < 37; k++) m.insert(k * 3);
  const seen = [...m.positions_seeded(99n)].sort((a, b) => a - b);
  assertEquals(seen, Array.from({ length: 37 }, (_, i) => i));
});

Deno.test("IndexedMap - clear forgets every entry and the next inserts start from zero", () => {
  const m = new IndexedMap(U32_KEYS, 4);
  for (let k = 0; k < 40; k++) m.insert(k);
  m.clear();
  assertEquals(m.size, 0);
  assertEquals(m.find(3), -1);
  assertEquals(m.insert(7), 0);
  assertEquals(m.insert(3), 1);
  assertEquals(m.find(7), 0);
});
