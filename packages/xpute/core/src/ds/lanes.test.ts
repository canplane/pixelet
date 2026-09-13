// @xpute/core/ds/lanes.test.ts

import { assertEquals } from "@std/assert";

import { Lanes } from "./lanes.ts";

Deno.test("lanes - rows are claimed, written, swap-removed and the moved row is reported", () => {
  const t = new Lanes({ id: Uint32Array, x: Float64Array, key: BigInt64Array }, { init_cap: 2 });
  for (let i = 0; i < 5; i++) {
    const row = t.push();
    t.mem.id[row] = i * 10;
    t.mem.x[row] = i / 2;
    t.mem.key[row] = BigInt(-i);
  }
  assertEquals(t.len, 5);
  assertEquals(t.mem.id.length >= 5, true, "grew past init_cap");
  assertEquals(Array.from(t.mem.id.subarray(0, 5)), [0, 10, 20, 30, 40]);

  const from = t.swap_remove(1);
  assertEquals(from, 4);
  assertEquals(t.len, 4);
  assertEquals(t.mem.id[1], 40);
  assertEquals(t.mem.x[1], 2);
  assertEquals(t.mem.key[1], -4n);

  assertEquals(t.swap_remove(3), 3);
  assertEquals(t.len, 3);
  t.truncate(1);
  assertEquals(t.len, 1);
  assertEquals(t.mem.id[0], 0);
});
