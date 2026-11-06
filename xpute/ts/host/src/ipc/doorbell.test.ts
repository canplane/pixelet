// @xpute/host/ipc/doorbell.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { Doorbell } from "./doorbell.ts";
import * as frame from "./frame.ts";
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
  door.listen(() => {
    assert.throws(() => door.call(1, null, (result) => result), InvariantError);
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

test("doorbell - a guest that traps throws at no one: it is told, and a call answers", () => {
  const sq_at = 8;
  const cq_at = sq_at + ring_bytes(CAP);
  const slots_at = cq_at + ring_bytes(CAP);
  const mem = new ArrayBuffer(slots_at + 2 * slots_bytes(CAP, SLOT));
  Ring.init(mem, sq_at, CAP, slots_at, SLOT);
  Ring.init(mem, cq_at, CAP, slots_at + slots_bytes(CAP, SLOT), SLOT);

  const error = console.error;
  console.error = () => {};
  try {
    let runs = 0;
    let terminated = 0;
    const door = new Doorbell(mem, sq_at, cq_at, () => {
      runs++;
      throw new Error("the guest trapped");
    });
    door.onterminated = () => terminated++;
    const answer = (result: number) => result;
    // In the call that traps, and in one after it, which rings no more.
    assert.deepStrictEqual(door.call(1, null, answer), Errno.ENOTRECOVERABLE);
    assert.deepStrictEqual(door.call(1, null, answer), Errno.ENOTRECOVERABLE);
    door.ring(0);
    assert.deepStrictEqual(runs, 1);
    assert.deepStrictEqual(terminated, 1);
  } finally {
    console.error = error;
  }
});

test("doorbell - a frame the host cannot take stops the guest, and it is told once", () => {
  const sq_at = 8;
  const cq_at = sq_at + ring_bytes(CAP);
  const slots_at = cq_at + ring_bytes(CAP);
  const mem = new ArrayBuffer(slots_at + 2 * slots_bytes(CAP, SLOT));
  Ring.init(mem, sq_at, CAP, slots_at, SLOT);
  const cq = Ring.init(
    mem,
    cq_at,
    CAP,
    slots_at + slots_bytes(CAP, SLOT),
    SLOT,
  );

  const error = console.error;
  console.error = () => {};
  try {
    let runs = 0;
    let terminated = 0;
    const door = new Doorbell(mem, sq_at, cq_at, () => {
      runs++;
      cq.push(0, 2);
      return 0;
    });
    door.listen(() => {
      throw new InvariantError(Errno.EBADF);
    });
    door.onterminated = () => terminated++;
    assert.deepStrictEqual(door.ring(0), -1);
    assert.deepStrictEqual(door.ring(0), -1);
    assert.deepStrictEqual(runs, 1, "a stopped guest is rung no more");
    assert.deepStrictEqual(terminated, 1);
  } finally {
    console.error = error;
  }
});

test("doorbell - a call answered in the turn that traps takes its answer once", () => {
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

  const error = console.error;
  console.error = () => {};
  try {
    // A guest that answers the call, then traps in the same turn.
    const door = new Doorbell(mem, sq_at, cq_at, () => {
      const at = sq.peek();
      const tag = sq.tag(at);
      sq.advance();
      cq.push(tag, 1, frame.RES, null, 7);
      throw new Error("the guest trapped");
    });
    const taken: number[] = [];
    const answer = door.call(1, null, (result) => (taken.push(result), result));
    assert.deepStrictEqual(answer, 7);
    assert.deepStrictEqual(taken, [7], "the answer is taken once");
  } finally {
    console.error = error;
  }
});

test("doorbell - a turn rung is heard before its wake", () => {
  const sq_at = 8;
  const cq_at = sq_at + ring_bytes(CAP);
  const slots_at = cq_at + ring_bytes(CAP);
  const mem = new ArrayBuffer(slots_at + 2 * slots_bytes(CAP, SLOT));
  Ring.init(mem, sq_at, CAP, slots_at, SLOT);
  Ring.init(mem, cq_at, CAP, slots_at + slots_bytes(CAP, SLOT), SLOT);
  // A guest on its own thread, whose rung turn asks for the next in 20 ms.
  const door = new Doorbell(
    mem,
    sq_at,
    cq_at,
    () => 20,
    () => {},
  );
  const heard: string[] = [];
  door.on_turn = (_turn_ms, wake_ms) => heard.push(`turn ${wake_ms}`);
  door.wake = (wake_ms) => heard.push(`wake ${wake_ms}`);
  door.ring(0);
  assert.deepStrictEqual(heard, ["turn 20", "wake 20"]);
});
