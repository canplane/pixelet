// @xpute/runtime/runtime.test.ts

/** Tests what `mount` puts together from its config alone: the front laid
 * before any turn, the core's commands handed out by their major, the stop,
 * and the turns paused and resumed. */

import { test } from "vitest";
import assert from "node:assert/strict";

import * as Errno from "@xpute/kit/status/errno.spec.ts";
import type { XputeError } from "@xpute/kit/status/error.ts";
import { type Core, mount, QUIET } from "./runtime.ts";
import type { Host } from "./io/connection.ts";
import * as frame from "./io/frame.ts";
import type { Front } from "./io/head.ts";
import type { Grant, SchedulingPolicy } from "./sched/policy.ts";

const PKT_BYTES = 256;
const FRAME = frame.WORD_BYTES + PKT_BYTES;
const HEAD_AT = 0;
/** The core's GPU stream begins here, as the header names it. */
const STREAM_AT = 32;
const RECEIVE_AT = 64;
const SEND_AT = RECEIVE_AT + FRAME;
const FRONT: Front = { pkt_bytes: PKT_BYTES, stream_at: STREAM_AT };
/** The core sends these two commands each turn: one of the program's, and one
 * of xpute's own. */
const PROGRAM_IMPORT = (frame.PROGRAM_MAJOR << 8) | 9;
const OWN_IMPORT = 0x0109;

/** This core reaches its runtime through the host it is handed. Each turn it
 * replies 0 to every frame it receives and sends PROGRAM_IMPORT and
 * OWN_IMPORT. When `trap` says so, the turn throws instead. */
function core(trap: () => boolean = () => false): Core & { turns: number[] } {
  const mem = new ArrayBuffer(SEND_AT + FRAME);
  let host: Host | null = null;
  const turns: number[] = [];
  const send = (tag: number, cmd: number, flags: number) => {
    frame.lay_frame(mem, SEND_AT, { tag, cmd, flags, result: 0, pkt: null });
    host!.send(SEND_AT, 0, 0);
  };
  return {
    mem,
    turns,
    run(deadline_ms) {
      turns.push(deadline_ms);
      if (trap()) throw new Error("the core trapped");
      while (host!.receive(RECEIVE_AT, FRAME) > 0) {
        const f = frame.frame_at(mem, RECEIVE_AT);
        send(f.tag, f.cmd, frame.RES);
      }
      send(0, PROGRAM_IMPORT, 0);
      send(0, OWN_IMPORT, 0);
      return -1;
    },
    host_with(h) {
      host = h;
    },
  };
}

/** This says where the front is, and that no GPU is asked for. */
const AT = { ...FRONT, head_at: HEAD_AT, gpu: null };

/** This policy's moments come when the test says, each a turn with a deadline
 * of 0. */
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
  const runtime = mount(
    g,
    AT,
    {
      program: [
        {
          majors: [PROGRAM_IMPORT >> 8],
          on: (_tag, cmd) => void cmds.push(cmd),
        },
      ],
      policy: stepped(),
      submit: () => {},
    },
    QUIET,
  );
  assert.deepStrictEqual(g.turns, [], "no turn before the front is laid");
  assert.deepStrictEqual(
    [runtime.head.pkt_bytes, runtime.head.stream_at],
    [PKT_BYTES, STREAM_AT],
  );
  runtime.send(PROGRAM_IMPORT);
  runtime.connection.run(0);
  assert.deepStrictEqual(cmds, [PROGRAM_IMPORT]);
});

test("mount - a front already laid is kept as it is", () => {
  const g = core();
  mount(g, AT, { policy: stepped(), submit: () => {} }, QUIET);
  const again = mount(
    g,
    { ...AT, stream_at: STREAM_AT + 8 },
    { policy: stepped(), submit: () => {} },
    QUIET,
  );
  assert.deepStrictEqual(
    again.head.stream_at,
    STREAM_AT,
    "laid again, it would move",
  );
});

test("mount - turns run between resume and pause, and wake asks for one only then", () => {
  const policy = stepped();
  const runtime = mount(core(), AT, { policy, submit: () => {} }, QUIET);
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

test("mount - a core that stops runs no more turns and is told once", async () => {
  const log = console.log;
  console.log = () => {};
  try {
    let trapping = false;
    let terminated = 0;
    const policy = stepped();
    const runtime = mount(
      core(() => trapping),
      AT,
      { policy, submit: () => {} },
      { ...QUIET, stop: () => terminated++ },
    );
    trapping = true;
    const answer = runtime.request(PROGRAM_IMPORT);
    runtime.connection.run(0);
    await assert.rejects(
      answer,
      (e: XputeError) => e.errnum === Errno.ENOTRECOVERABLE,
    );
    runtime.resume();
    assert.deepStrictEqual(terminated, 1);
    assert.deepStrictEqual(
      policy.asked,
      0,
      "a stopped core is resumed no more",
    );
  } finally {
    console.log = log;
  }
});

test("mount - runs after_turn as each turn returns", () => {
  let ran = 0;
  const runtime = mount(
    core(),
    AT,
    { after_turn: () => void ran++, policy: stepped(), submit: () => {} },
    QUIET,
  );
  assert.deepStrictEqual(ran, 0);
  runtime.connection.run(0);
  assert.deepStrictEqual(ran, 1);
});
