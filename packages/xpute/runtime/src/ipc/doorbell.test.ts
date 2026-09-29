// @xpute/runtime/ipc/doorbell.test.ts

import { assertEquals, assertThrows } from "@std/assert";

import { InvariantError } from "@xpute/core/status/error.ts";
import { Doorbell } from "./doorbell.ts";
import { Ring, ring_bytes, slots_bytes } from "./ring.ts";

const CAPACITY = 8;
const SLOT = 64;

Deno.test("doorbell - a call from inside a listener is refused and leaves nothing submitted", () => {
  const sq_at = 8;
  const cq_at = sq_at + ring_bytes(CAPACITY);
  const slots_at = cq_at + ring_bytes(CAPACITY);
  const mem = new ArrayBuffer(slots_at + 2 * slots_bytes(CAPACITY, SLOT));
  const sq = Ring.init(mem, sq_at, CAPACITY, slots_at, SLOT);
  const cq = Ring.init(mem, cq_at, CAPACITY, slots_at + slots_bytes(CAPACITY, SLOT), SLOT);

  // A guest whose every turn raises one signal.
  const door = new Doorbell(mem, sq_at, cq_at, () => {
    cq.push(0, 2);
    return -1;
  });
  let refused = 0;
  door.signal = () => {
    assertThrows(() => door.call(1, null, (result) => result), InvariantError);
    refused++;
  };
  door.ring(0);
  assertEquals(refused, 1);
  assertEquals(sq.len, 0, "the refused call is not in the submission ring");
});
