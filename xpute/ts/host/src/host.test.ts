// @xpute/host/host.test.ts

/** What `mount` puts together from its config alone: the first turn where it
 * writes the toc, the ports, the stop, and the turns paused and resumed. */

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import { InvariantError } from "@xpute/kit/status/error.ts";
import { type Guest, mount } from "./host.ts";
import * as frame from "./ipc/frame.ts";
import { Ring, ring_bytes, slots_bytes } from "./ipc/ring.ts";
import { HEAD, key, MAGIC, VERSION } from "./ipc/toc.spec.ts";
import type { Grant, SchedulingPolicy } from "./sched/policy.ts";

const CAP = 8;
const SLOT = 256;
const TOC_AT = 8;
const SQ_AT = 64;
const CQ_AT = SQ_AT + ring_bytes(CAP);
const SLOTS_AT = CQ_AT + ring_bytes(CAP);
/** A stream of the program's own, which the guest names in its toc. */
const STREAM_KEY = (key.PROGRAM | 1) >>> 0;
const STREAM_AT = 4;

/** A guest that lays its rings and writes its toc in its first turn, then
 * each turn answers what was sent it, 0, and posts an event for port 1;
 * `trap` makes a turn throw. */
function guest(trap: () => boolean = () => false): Guest & { turns: number[] } {
  const mem = new ArrayBuffer(SLOTS_AT + 2 * slots_bytes(CAP, SLOT));
  let sq: Ring | null = null;
  let cq: Ring | null = null;
  const turns: number[] = [];
  return {
    mem,
    turns,
    run(quota_ms) {
      turns.push(quota_ms);
      if (sq === null || cq === null) {
        sq = Ring.init(mem, SQ_AT, CAP, SLOTS_AT, SLOT);
        cq = Ring.init(
          mem,
          CQ_AT,
          CAP,
          SLOTS_AT + slots_bytes(CAP, SLOT),
          SLOT,
        );
        const toc = new Uint32Array(mem, TOC_AT, HEAD + 6);
        toc.set([
          MAGIC,
          VERSION,
          3,
          0,
          key.SQ,
          SQ_AT,
          key.CQ,
          CQ_AT,
          STREAM_KEY,
          STREAM_AT,
        ]);
        return -1;
      }
      if (trap()) throw new Error("the guest trapped");
      for (let at = sq.peek(); at >= 0; at = sq.peek()) {
        cq.push(sq.tag(at), sq.cmd(at), frame.RES);
        sq.advance();
      }
      cq.push(1 << 24, 9);
      return -1;
    },
    answer_with() {},
  };
}

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

test("mount - runs the turn that writes the toc, and binds each port before any turn is read", () => {
  const g = guest();
  const heard: number[] = [];
  const host = mount(g, {
    toc_at: TOC_AT,
    toc_in_turn: true,
    sys: {},
    ports: { 1: (_tag, cmd) => void heard.push(cmd) },
    policy: stepped(),
    submit: () => {},
  });
  assert.deepStrictEqual(
    g.turns,
    [0],
    "one turn, of no quota, before the toc is read",
  );
  host.call(1, 0);
  assert.deepStrictEqual(heard, [9]);
});

test("mount - refuses a handler for the host's own port", () => {
  assert.throws(
    () =>
      mount(guest(), {
        toc_at: TOC_AT,
        toc_in_turn: true,
        sys: {},
        ports: { 0: () => {} },
        policy: stepped(),
        submit: () => {},
      }),
    (e: unknown) => e instanceof InvariantError && e.errnum === Errno.EINVAL,
  );
});

test("mount - turns run between resume and pause, and wake asks for one only then", () => {
  const policy = stepped();
  const host = mount(guest(), {
    toc_at: TOC_AT,
    toc_in_turn: true,
    sys: {},
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
        toc_at: TOC_AT,
        toc_in_turn: true,
        sys: {},
        policy,
        submit: () => {},
        onterminated: () => terminated++,
      },
    );
    trapping = true;
    assert.deepStrictEqual(host.call(1, 0), Errno.ENOTRECOVERABLE);
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

test("mount - runs each stream the platform names as each turn returns", () => {
  const g = guest();
  const ran: number[] = [];
  g.streams = [{ key: STREAM_KEY, run: (at) => void ran.push(at) }];
  const host = mount(g, {
    toc_at: TOC_AT,
    toc_in_turn: true,
    sys: {},
    policy: stepped(),
    submit: () => {},
  });
  assert.deepStrictEqual(ran, [], "not the turn that wrote the toc");
  host.call(1, 0);
  assert.deepStrictEqual(ran, [STREAM_AT]);
});
