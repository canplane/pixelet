// @pxl/web/wasm/ring.test.ts

import { assert, assertEquals } from "@std/assert";

import { Errno } from "@xpute/core/sys/errno.ts";
import { Ring, RING_OP, RING_RESULT, RING_USER } from "@xpute/core/ipc/ring.ts";

import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";

/** An op crates/pxl-wasm/src/lib.rs does not number. */
const UNNUMBERED = 0xfff;

Deno.test("rings - the glue submits, one drain completes each in order, and a full completion ring holds the rest back", () => {
  assert(CORE_PXL !== null);
  const p = CORE_PXL;
  const sq = new Ring(p.memory.buffer, p.pxl_ring_sq());
  const cq = new Ring(p.memory.buffer, p.pxl_ring_cq());
  assertEquals([sq.capacity, cq.capacity], [1024, 1024]);

  const n = sq.capacity;
  for (let k = 0; k < n; k++) assertEquals(sq.push(k, 0, k * 3), Errno.OK);
  assertEquals(sq.push(n, 0), Errno.EAGAIN, "a full submission ring refuses");
  assertEquals(p.pxl_drain(), n);
  assertEquals(sq.len, 0);

  // The completion ring is now full: new submissions wait in theirs.
  for (let k = 0; k < 5; k++) sq.push(1000 + k, UNNUMBERED, 0);
  assertEquals(p.pxl_drain(), 0, "nothing completes into a full ring");
  assertEquals(sq.len, 5, "and nothing is dropped");

  for (let k = 0; k < n; k++) {
    const at = cq.peek();
    assertEquals([cq.words[at + RING_USER], cq.words[at + RING_RESULT]], [k, k * 3]);
    cq.advance();
  }
  assertEquals(p.pxl_drain(), 5);
  const at = cq.peek();
  assertEquals([cq.words[at + RING_USER], cq.words[at + RING_OP], cq.words[at + RING_RESULT] | 0], [1000, UNNUMBERED, Errno.ENOSYS]);
  while (cq.peek() >= 0) cq.advance();
});
