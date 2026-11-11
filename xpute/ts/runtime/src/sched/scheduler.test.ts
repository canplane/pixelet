// @xpute/runtime/sched/scheduler.test.ts

/** Every wake the core asks for, at once or in `n` ms, is run only at a
 * moment the policy gives and admits. The connection and the policy are
 * stand-ins that record the turns and the moments asked for. */

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import type { Connection } from "../io/connection.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";
import { Scheduler } from "./scheduler.ts";
import { NO_WAKE } from "./wake.ts";

afterEach(() => {
  vi.useRealTimers();
});

function setup() {
  const kicks: number[] = [];
  const moments: ((now: number) => void)[] = [];
  const connection = {
    terminated: false,
    run: (deadline_ms: number) => (kicks.push(deadline_ms), -1),
  };
  const policy: SchedulingPolicy = {
    next: (consider) => moments.push(consider),
    admit: () => true,
    grant: (): Grant => ({ deadline: Infinity }),
    observe: () => {},
  };
  const scheduler = new Scheduler(
    connection as unknown as Connection,
    policy,
    () => {},
  );
  /** Hears a turn the scheduler did not kick, as the connection tells it: first
   * the turn, then its wake. */
  const heard = (wake_ms: number) => {
    scheduler.on_turn();
    scheduler.on_wake(wake_ms);
  };
  const wake = (wake_ms: number) => scheduler.on_wake(wake_ms);
  const moment = () => moments.shift()?.(performance.now());
  return { scheduler, wake, heard, kicks, moments, moment };
}

test("a thread core's timed wake waits for its time and the policy's moment", () => {
  vi.useFakeTimers();
  const { scheduler, wake, kicks, moments, moment } = setup();
  scheduler.resume();
  moment();
  assert.deepEqual(kicks, [Infinity], "resumed, a turn at the first moment");

  wake(20);
  vi.advanceTimersByTime(19);
  assert.equal(moments.length, 0, "no moment asked before the wake's time");
  vi.advanceTimersByTime(1);
  assert.equal(moments.length, 1, "the moment asked at the wake's time");
  assert.deepEqual(kicks, [Infinity], "and no kick before the policy gives it");
  moment();
  assert.deepEqual(
    kicks,
    [Infinity, Infinity],
    "kicked at the moment, with its grant",
  );
});

test("a thread core's wake at once waits for the policy's moment", () => {
  const { scheduler, wake, kicks, moments, moment } = setup();
  scheduler.resume();
  moment();
  wake(0);
  assert.equal(moments.length, 1);
  assert.deepEqual(kicks, [Infinity]);
  moment();
  assert.deepEqual(kicks, [Infinity, Infinity]);
});

test("what the runtime sends a thread core is granted a turn", () => {
  const { scheduler, kicks, moments, moment } = setup();
  scheduler.resume();
  moment();
  scheduler.wake();
  assert.equal(moments.length, 1);
  moment();
  assert.deepEqual(kicks, [Infinity, Infinity]);
});

test("paused, a thread core's timed wake asks nothing", () => {
  vi.useFakeTimers();
  const { scheduler, wake, moments, moment } = setup();
  scheduler.resume();
  moment();
  wake(20);
  scheduler.pause();
  vi.advanceTimersByTime(40);
  assert.equal(moments.length, 0);
});

test("a turn heard that asks for nothing lets the held timer go", () => {
  vi.useFakeTimers();
  const { scheduler, wake, heard, moments, moment } = setup();
  scheduler.resume();
  moment();
  wake(20);
  heard(NO_WAKE);
  vi.advanceTimersByTime(40);
  assert.equal(moments.length, 0, "no turn for the earlier wake");
});

test("a turn heard that asks for later is paced at its own wake", () => {
  vi.useFakeTimers();
  const { scheduler, wake, heard, moments, moment } = setup();
  scheduler.resume();
  moment();
  wake(20);
  heard(50);
  vi.advanceTimersByTime(49);
  assert.equal(moments.length, 0, "not at the earlier wake's time");
  vi.advanceTimersByTime(1);
  assert.equal(moments.length, 1, "at the later wake's time");
});

test("a moment asked before a turn heard runs nothing", () => {
  const { scheduler, wake, heard, kicks, moments, moment } = setup();
  scheduler.resume();
  moment();
  wake(0);
  heard(NO_WAKE);
  assert.equal(
    moments.length,
    1,
    "the moment asked is the policy's to hand back",
  );
  moment();
  assert.deepEqual(kicks, [Infinity], "and it runs no turn");
  assert.equal(moments.length, 0, "nor asks another");
});

test("a turn the runtime wants survives a turn heard", () => {
  const { scheduler, heard, kicks, moments } = setup();
  scheduler.resume();
  moments.shift()?.(performance.now());
  scheduler.wake();
  heard(NO_WAKE);
  for (const m of moments.splice(0)) m(performance.now());
  assert.deepEqual(
    kicks,
    [Infinity, Infinity],
    "the runtime's turn runs at the new moment",
  );
});
