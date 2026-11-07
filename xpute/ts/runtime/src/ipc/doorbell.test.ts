// @xpute/runtime/ipc/doorbell.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { Scheduler } from "../sched/scheduler.ts";
import { NO_WAKE } from "../sched/wake.ts";
import { Doorbell } from "./doorbell.ts";
import * as frame from "./frame.ts";
import { Ring, ring_bytes, slots_bytes } from "./ring.ts";

const CAP = 8;
const SLOT = 64;

/** A core's rings, laid in fresh memory: the SQ, the CQ, then their slots. */
function rings(): {
  mem: ArrayBuffer;
  sq: Ring;
  cq: Ring;
  sq_at: number;
  cq_at: number;
} {
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
  return { mem, sq, cq, sq_at, cq_at };
}

test("doorbell - a call from inside a listener is refused and leaves nothing submitted", () => {
  const { mem, sq, cq, sq_at, cq_at } = rings();

  // A core whose every turn raises one import's frame.
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

test("doorbell - a core that traps throws at no one: it is told, and a call answers", () => {
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
      throw new Error("the core trapped");
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

test("doorbell - a frame the runtime cannot take stops the core, and it is told once", () => {
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
    assert.deepStrictEqual(runs, 1, "a stopped core is rung no more");
    assert.deepStrictEqual(terminated, 1);
  } finally {
    console.error = error;
  }
});

test("doorbell - a call answered in the turn that traps takes its answer once", () => {
  const { mem, sq, cq, sq_at, cq_at } = rings();

  const error = console.error;
  console.error = () => {};
  try {
    // A core that answers the call, then traps in the same turn.
    const door = new Doorbell(mem, sq_at, cq_at, () => {
      const at = sq.peek();
      const tag = sq.tag(at);
      sq.advance();
      cq.push(tag, 1, frame.RES, null, 7);
      throw new Error("the core trapped");
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
  // A core on its own thread, whose rung turn asks for the next in 20 ms.
  const door = new Doorbell(
    mem,
    sq_at,
    cq_at,
    () => 20,
    () => {},
  );
  const heard: string[] = [];
  door.on_turn = (_turn_ms, wake_ms) => heard.push(`turn ${wake_ms}`);
  door.on_wake = (wake_ms) => heard.push(`wake ${wake_ms}`);
  door.ring(0);
  assert.deepStrictEqual(heard, ["turn 20", "wake 20"]);
});

test("doorbell - a rung core's read asks the next turn, which the turn heard lets stand", () => {
  const { mem, cq, sq_at, cq_at } = rings();
  let turns = 0;
  // A core with no wake of its own, whose first turn posts a frame.
  const door = new Doorbell(mem, sq_at, cq_at, () => {
    if (++turns === 1) cq.push(0, 2);
    return NO_WAKE;
  });
  const moments: ((now: number) => void)[] = [];
  const scheduler = new Scheduler(
    door,
    {
      next: (consider) => moments.push(consider),
      admit: () => true,
      grant: () => ({ deadline: Infinity }),
      observe: () => {},
    },
    () => {},
  );
  scheduler.resume();
  moments.shift()!(0);
  assert.deepStrictEqual(turns, 1);
  moments.shift()!(0);
  assert.deepStrictEqual(turns, 2, "a task may wait on the head passing");
  assert.deepStrictEqual(
    moments.length,
    0,
    "a turn that read nothing asks none",
  );
});

test("doorbell - a request is answered by the reply its turn posts, and waits for no turn", async () => {
  const { mem, sq, cq, sq_at, cq_at } = rings();
  let turns = 0;
  // A core that replies to each submission with ACKREQ as it applies it.
  const door = new Doorbell(mem, sq_at, cq_at, () => {
    turns++;
    for (let at = sq.peek(); at >= 0; at = sq.peek()) {
      const tag = sq.tag(at);
      const acked = sq.flags(at) & frame.ACKREQ;
      sq.advance();
      if (acked) cq.push(tag, 1, frame.RES, null, 40 + tag);
    }
    return -1;
  });
  const answer = door.request(1);
  assert.deepStrictEqual(turns, 0, "requesting rings nothing");
  door.ring(0);
  assert.deepStrictEqual(await answer, 41);
});

test("doorbell - a request still waiting as the core stops is answered ENOTRECOVERABLE", async () => {
  const { mem, sq_at, cq_at } = rings();
  const error = console.error;
  console.error = () => {};
  try {
    const door = new Doorbell(mem, sq_at, cq_at, () => {
      throw new Error("the core trapped");
    });
    const answer = door.request(1);
    door.ring(0);
    assert.deepStrictEqual(await answer, Errno.ENOTRECOVERABLE);
    assert.deepStrictEqual(await door.request(1), Errno.ENOTRECOVERABLE);
  } finally {
    console.error = error;
  }
});
