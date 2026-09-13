// @xpute/core/ipc/ring.test.ts

import { assert, assertEquals } from "@std/assert";

import { Errno } from "@xpute/core/sys/errno.ts";
import { Ring, RING_ARGS, RING_USER, ring_words } from "./ring.ts";

Deno.test("ring - entries come out in order across the wrap, and a full ring refuses", () => {
  const buf = new ArrayBuffer(ring_words(4) * 4 + 8);
  const r = Ring.init(buf, 8, 4);
  for (let round = 0; round < 3; round++) {
    for (let k = 0; k < 4; k++) assertEquals(r.push(round * 4 + k, 0, (round * 4 + k) * 10), Errno.OK);
    assertEquals(r.push(99, 0), Errno.EAGAIN, "a full ring refuses");
    for (let k = 0; k < 4; k++) {
      const at = r.peek();
      assert(at >= 0);
      assertEquals([r.words[at + RING_USER], r.words[at + RING_ARGS]], [round * 4 + k, (round * 4 + k) * 10]);
      r.advance();
    }
    assertEquals(r.peek(), -1);
  }
  assertEquals([r.head, r.tail], [12, 12], "positions only grow");
});
