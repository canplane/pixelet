// @xpute/runtime/runtime.test.ts

/** What `mount` puts together from its config alone: the front laid before
 * any turn, the core's commands handed by their major, the stop, and the
 * turns paused and resumed. */

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { type Config, type Core, mount } from "./runtime.ts";
import * as frame from "./ipc/frame.ts";
import type { Front } from "./ipc/head.ts";
import { Ring, ring_bytes, slots_bytes } from "./ipc/ring.ts";
import * as ring from "./ipc/ring.spec.ts";
import type { Grant, SchedulingPolicy } from "./sched/policy.ts";

const CAP = 8;
const SLOT = 256;
const HEAD_AT = 0;
/** Where the core's GPU stream begins, which the header names. */
const STREAM_AT = 32;
const SQ_AT = 64;
const CQ_AT = SQ_AT + ring_bytes(CAP);
const SLOTS_AT = CQ_AT + ring_bytes(CAP);
const FRONT: Front = {
  sq_cap: CAP,
  cq_cap: CAP,
  slot_bytes: SLOT,
  sq_at: SQ_AT,
  cq_at: CQ_AT,
  sq_slots_at: SLOTS_AT,
  cq_slots_at: SLOTS_AT + slots_bytes(CAP, SLOT),
  gpu_at: STREAM_AT,
};
/** A command of the program's, and one of xpute's own, the core posts
 * each turn. */
const PROGRAM_EVENT = (frame.PROGRAM_MAJOR << 8) | 9;
const OWN_EVENT = 0x0109;

/** A core that opens the rings its runtime laid, then each turn answers what
 * was sent it, 0, and posts PROGRAM_EVENT and OWN_EVENT; `trap` makes a turn
 * throw. */
function core(trap: () => boolean = () => false): Core & { turns: number[] } {
  const mem = new ArrayBuffer(FRONT.cq_slots_at + slots_bytes(CAP, SLOT));
  let sq: Ring | null = null;
  let cq: Ring | null = null;
  const turns: number[] = [];
  return {
    mem,
    turns,
    run(deadline_ms) {
      turns.push(deadline_ms);
      sq ??= new Ring(mem, SQ_AT);
      cq ??= new Ring(mem, CQ_AT);
      if (trap()) throw new Error("the core trapped");
      for (let at = sq.peek(); at >= 0; at = sq.peek()) {
        cq.push(sq.tag(at), sq.cmd(at), frame.RES);
        sq.advance();
      }
      cq.push(0, PROGRAM_EVENT);
      cq.push(0, OWN_EVENT);
      return -1;
    },
    answer_with() {},
  };
}

const LAID = { head_at: HEAD_AT, lay: FRONT } satisfies Partial<Config>;

/** A policy whose moments come when the test says, each a turn with a deadline of 0. */
function stepped(): SchedulingPolicy & { asked: number; step(): void } {
  let waiting: ((now: number) => void) | null = null;
  return {
    asked: 0,
    next(consider) {
      this.asked++;
      waiting = consider;
    },
    admit: () => true,
    grant: (): Grant => ({ deadline: 0 }),
    observe() {},
    step() {
      const w = waiting;
      waiting = null;
      w?.(0);
    },
  };
}

test("mount - lays the front before any turn, and hands the program only its own commands", () => {
  const g = core();
  const cmds: number[] = [];
  const runtime = mount(g, {
    ...LAID,
    program: (_tag, cmd) => void cmds.push(cmd),
    policy: stepped(),
    submit: () => {},
  });
  assert.deepStrictEqual(g.turns, [], "no turn before the front is laid");
  assert.deepStrictEqual(
    [runtime.head.sq_at, runtime.head.cq_at, runtime.head.gpu_at],
    [SQ_AT, CQ_AT, STREAM_AT],
  );
  runtime.call(PROGRAM_EVENT);
  assert.deepStrictEqual(cmds, [PROGRAM_EVENT]);
});

test("mount - a front already laid is kept as it is, its rings' positions with it", () => {
  const g = core();
  mount(g, { ...LAID, policy: stepped(), submit: () => {} });
  const sq = new Uint32Array(g.mem, SQ_AT, ring.HEADER_WORDS);
  const tail = sq[ring.TAIL];
  assert.ok(tail > 0, "the attach is waiting in the submission ring");
  mount(g, { ...LAID, policy: stepped(), submit: () => {} });
  assert.ok(sq[ring.TAIL] > tail, "laid again, the ring would start over");
});

test("mount - turns run between resume and pause, and wake asks for one only then", () => {
  const policy = stepped();
  const runtime = mount(core(), {
    ...LAID,
    policy,
    submit: () => {},
  });
  runtime.wake();
  assert.deepStrictEqual(policy.asked, 0, "paused, so no turn is asked for");
  runtime.resume();
  runtime.resume();
  assert.deepStrictEqual(policy.asked, 1, "a second resume asks nothing more");
  policy.step();
  const asked = policy.asked;
  runtime.pause();
  runtime.wake();
  assert.deepStrictEqual(policy.asked, asked, "paused, so wake asks nothing");
});

test("mount - a core that stops runs no more turns and is told once", () => {
  const error = console.error;
  console.error = () => {};
  try {
    let trapping = false;
    let terminated = 0;
    const policy = stepped();
    const runtime = mount(
      core(() => trapping),
      {
        ...LAID,
        policy,
        submit: () => {},
        onterminated: () => terminated++,
      },
    );
    trapping = true;
    assert.deepStrictEqual(runtime.call(PROGRAM_EVENT), Errno.ENOTRECOVERABLE);
    runtime.resume();
    assert.deepStrictEqual(terminated, 1);
    assert.deepStrictEqual(
      policy.asked,
      0,
      "a stopped core is resumed no more",
    );
  } finally {
    console.error = error;
  }
});

test("mount - grants the GPU the core's stream and memory, and runs it as each turn returns", () => {
  const granted: number[] = [];
  let ran = 0;
  const gpu = {
    grant: (to: { at: number }) => void granted.push(to.at),
    after_turn: () => void ran++,
  };
  const ask = {
    power: "high-performance",
    features: { required: [], optional: [] },
  };
  const runtime = mount(core(), {
    ...LAID,
    gpu: { by: gpu, ask },
    policy: stepped(),
    submit: () => {},
  });
  assert.deepStrictEqual([granted, ran], [[STREAM_AT], 0]);
  runtime.call(PROGRAM_EVENT);
  assert.deepStrictEqual(ran, 1);
});
