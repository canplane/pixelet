// @xpute/host/ipc/doorbell.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { InvariantError } from "@xpute/kit/status/error.ts";
import { Doorbell } from "./doorbell.ts";
import { Ring, ring_bytes, slots_bytes } from "./ring.ts";

const CAP = 8;
const SLOT = 64;

test("doorbell - a call from inside a listener is refused and leaves nothing submitted", () => {
  const sq_at = 8;
  const cq_at = sq_at + ring_bytes(CAP);
  const slots_at = cq_at + ring_bytes(CAP);
  const mem = new ArrayBuffer(slots_at + 2 * slots_bytes(CAP, SLOT));
  const sq = Ring.init(mem, sq_at, CAP, slots_at, SLOT);
  const cq = Ring.init(
    mem,
    cq_at,
    CAP,
    slots_at + slots_bytes(CAP, SLOT),
    SLOT,
  );

  // A guest whose every turn raises one event.
  const door = new Doorbell(mem, sq_at, cq_at, () => {
    cq.push(0, 2);
    return -1;
  });
  let refused = 0;
  door.listen(0, () => {
    assert.throws(
      () => door.call(1, 1, null, (result) => result),
      InvariantError,
    );
    refused++;
  });
  door.ring(0);
  assert.deepStrictEqual(refused, 1);
  assert.deepStrictEqual(
    sq.len,
    0,
    "the refused call is not in the submission ring",
  );
});
