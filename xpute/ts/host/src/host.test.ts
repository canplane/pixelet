// @xpute/host/host.test.ts

/** What `mount` puts together from its config alone: the front laid before
 * any turn, the guest's commands handed by their major, the stop, and the
 * turns paused and resumed. */

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { type Config, type Guest, mount } from "./host.ts";
import * as frame from "./ipc/frame.ts";
import type { Front } from "./ipc/head.ts";
import { Ring, ring_bytes, slots_bytes } from "./ipc/ring.ts";
import * as ring from "./ipc/ring.spec.ts";
import type { Grant, SchedulingPolicy } from "./sched/policy.ts";

const CAP = 8;
const SLOT = 256;
const HEAD_AT = 0;
/** Where the guest's GPU stream begins, which the header names. */
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
/** A command of the program's, and one of xpute's own, the guest posts
 * each turn. */
const PROGRAM_EVENT = (frame.PROGRAM_MAJOR << 8) | 9;
const OWN_EVENT = 0x0109;

/** A guest that opens the rings its host laid, then each turn answers what
 * was sent it, 0, and posts PROGRAM_EVENT and OWN_EVENT; `trap` makes a turn
 * throw. */
function guest(trap: () => boolean = () => false): Guest & { turns: number[] } {
  const mem = new ArrayBuffer(FRONT.cq_slots_at + slots_bytes(CAP, SLOT));
  let sq: Ring | null = null;
  let cq: Ring | null = null;
  const turns: number[] = [];
  return {
    mem,
    turns,
    run(quota_ms) {
      turns.push(quota_ms);
      sq ??= new Ring(mem, SQ_AT);
      cq ??= new Ring(mem, CQ_AT);
      if (trap()) throw new Error("the guest trapped");
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

/** A policy whose moments come when the test says, each a turn of no quota. */
function stepped(): SchedulingPolicy & { asked: number; step(): void } {
  let waiting: ((now: number) => void) | null = null;
  return {
    asked: 0,
    next(consider) {
      this.asked++;
      waiting = consider;
    },
    admit: () => true,
    grant: (): Grant => ({ quota_ms: 0 }),
    observe() {},
    step() {
      const w = waiting;
      waiting = null;
      w?.(0);
    },
  };
}

test("mount - lays the front before any turn, and hands the program only its own commands", () => {
  const g = guest();
  const heard: number[] = [];
  const host = mount(g, {
    ...LAID,
    program: (_tag, cmd) => void heard.push(cmd),
    policy: stepped(),
    submit: () => {},
  });
  assert.deepStrictEqual(g.turns, [], "no turn before the front is laid");
  assert.deepStrictEqual(
    [host.head.sq_at, host.head.cq_at, host.head.gpu_at],
    [SQ_AT, CQ_AT, STREAM_AT],
  );
  host.call(PROGRAM_EVENT);
  assert.deepStrictEqual(heard, [PROGRAM_EVENT]);
});

test("mount - a front already laid is kept as it is, its rings' positions with it", () => {
  const g = guest();
  mount(g, { ...LAID, policy: stepped(), submit: () => {} });
  const sq = new Uint32Array(g.mem, SQ_AT, ring.HEADER_WORDS);
  const tail = sq[ring.TAIL];
  assert.ok(tail > 0, "the attach is waiting in the submission ring");
  mount(g, { ...LAID, policy: stepped(), submit: () => {} });
  assert.ok(sq[ring.TAIL] > tail, "laid again, the ring would start over");
});

test("mount - turns run between resume and pause, and wake asks for one only then", () => {
  const policy = stepped();
  const host = mount(guest(), {
    ...LAID,
    policy,
    submit: () => {},
  });
  host.wake();
  assert.deepStrictEqual(policy.asked, 0, "paused, so no turn is asked for");
  host.resume();
  host.resume();
  assert.deepStrictEqual(policy.asked, 1, "a second resume asks nothing more");
  policy.step();
  host.pause();
  host.wake();
  assert.deepStrictEqual(policy.asked, 1);
});

test("mount - a guest that stops runs no more turns and is told once", () => {
  const error = console.error;
  console.error = () => {};
  try {
    let trapping = false;
    let terminated = 0;
    const policy = stepped();
    const host = mount(
      guest(() => trapping),
      {
        ...LAID,
        policy,
        submit: () => {},
        onterminated: () => terminated++,
      },
    );
    trapping = true;
    assert.deepStrictEqual(host.call(PROGRAM_EVENT), Errno.ENOTRECOVERABLE);
    host.resume();
    assert.deepStrictEqual(terminated, 1);
    assert.deepStrictEqual(
      policy.asked,
      0,
      "a stopped guest is resumed no more",
    );
  } finally {
    console.error = error;
  }
});

test("mount - grants the GPU the guest's stream and memory, and runs it as each turn returns", () => {
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
  const host = mount(guest(), {
    ...LAID,
    gpu: { by: gpu, ask },
    policy: stepped(),
    submit: () => {},
  });
  assert.deepStrictEqual([granted, ran], [[STREAM_AT], 0]);
  host.call(PROGRAM_EVENT);
  assert.deepStrictEqual(ran, 1);
});
