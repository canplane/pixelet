// @xpute/runtime/sched/scheduler.test.ts

/** The scheduler as the one that times a core on its own thread, which keeps
 * no timer: every wake the core asks, at once or in `n` ms, is kicked only
 * at a moment the policy gives and admits. The doorbell and the policy are
 * stand-ins that record the kicks and the moments asked. */

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import type { Doorbell } from "../ipc/doorbell.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";
import { Scheduler } from "./scheduler.ts";
import { NO_WAKE } from "./wake.ts";

afterEach(() => {
  vi.useRealTimers();
});

function setup() {
  const kicks: number[] = [];
  const moments: ((now: number) => void)[] = [];
  const doorbell = {
    terminated: false,
    wake: null as ((wake_ms: number) => void) | null,
    on_turn: null as ((turn_ms: number, wake_ms: number) => void) | null,
    kick: (deadline_ms: number) => kicks.push(deadline_ms),
    batch: (f: () => void) => f(),
  };
  const policy: SchedulingPolicy = {
    next: (consider) => moments.push(consider),
    admit: () => true,
    grant: (): Grant => ({ deadline: Infinity }),
    observe: () => {},
  };
  const scheduler = new Scheduler(
    doorbell as unknown as Doorbell,
    policy,
    () => {},
  );
  /** A turn the thread ran that the scheduler did not kick, a device's, heard
   * as `Doorbell.ack` hears it: the turn, then its wake. */
  const heard = (wake_ms: number) => {
    doorbell.on_turn!(1, wake_ms);
    doorbell.wake!(wake_ms);
  };
  /** The policy's next moment, given. */
  const moment = () => moments.shift()?.(performance.now());
  return { scheduler, wake: doorbell.wake!, heard, kicks, moments, moment };
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

test("a device-kicked turn that asks for nothing lets the held timer go", () => {
  vi.useFakeTimers();
  const { scheduler, wake, heard, moments, moment } = setup();
  scheduler.resume();
  moment();
  wake(20);
  heard(NO_WAKE);
  vi.advanceTimersByTime(40);
  assert.equal(moments.length, 0, "no turn for the earlier wake");
});

test("a device-kicked turn that asks for later is paced at its own wake", () => {
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
