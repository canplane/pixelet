// @xpute/host/sched/strobe.test.ts

/** The strobe as the one scheduler of a guest on its own thread, which keeps
 * no timer: every wake the guest asks, at once or in `n` ms, is kicked only
 * at a moment the policy gives and admits. The doorbell and the policy are
 * stand-ins that record the kicks and the moments asked. */

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import type { Doorbell } from "../ipc/doorbell.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";
import { Strobe } from "./strobe.ts";
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
    kick: (quota_ms: number) => kicks.push(quota_ms),
    batch: (f: () => void) => f(),
  };
  const policy: SchedulingPolicy = {
    next: (consider) => moments.push(consider),
    admit: () => true,
    grant: (): Grant => ({ quota_ms: 4 }),
    observe: () => {},
  };
  const strobe = new Strobe(doorbell as unknown as Doorbell, policy, () => {});
  /** A turn the thread ran that the strobe did not kick, a device's, heard
   * as `Doorbell.ack` hears it: the turn, then its wake. */
  const heard = (wake_ms: number) => {
    doorbell.on_turn!(1, wake_ms);
    doorbell.wake!(wake_ms);
  };
  /** The policy's next moment, given. */
  const moment = () => moments.shift()?.(performance.now());
  return { strobe, wake: doorbell.wake!, heard, kicks, moments, moment };
}

test("a thread guest's timed wake waits for its time and the policy's moment", () => {
  vi.useFakeTimers();
  const { strobe, wake, kicks, moments, moment } = setup();
  strobe.resume();
  moment();
  assert.deepEqual(kicks, [4], "resumed, a turn at the first moment");

  wake(20);
  vi.advanceTimersByTime(19);
  assert.equal(moments.length, 0, "no moment asked before the wake's time");
  vi.advanceTimersByTime(1);
  assert.equal(moments.length, 1, "the moment asked at the wake's time");
  assert.deepEqual(kicks, [4], "and no kick before the policy gives it");
  moment();
  assert.deepEqual(kicks, [4, 4], "kicked at the moment, with its grant");
});

test("a thread guest's wake at once waits for the policy's moment", () => {
  const { strobe, wake, kicks, moments, moment } = setup();
  strobe.resume();
  moment();
  wake(0);
  assert.equal(moments.length, 1);
  assert.deepEqual(kicks, [4]);
  moment();
  assert.deepEqual(kicks, [4, 4]);
});

test("paused, a thread guest's timed wake asks nothing", () => {
  vi.useFakeTimers();
  const { strobe, wake, moments, moment } = setup();
  strobe.resume();
  moment();
  wake(20);
  strobe.pause();
  vi.advanceTimersByTime(40);
  assert.equal(moments.length, 0);
});

test("a device-kicked turn that asks for nothing lets the held timer go", () => {
  vi.useFakeTimers();
  const { strobe, wake, heard, moments, moment } = setup();
  strobe.resume();
  moment();
  wake(20);
  heard(NO_WAKE);
  vi.advanceTimersByTime(40);
  assert.equal(moments.length, 0, "no turn for the earlier wake");
});

test("a device-kicked turn that asks for later is paced at its own wake", () => {
  vi.useFakeTimers();
  const { strobe, wake, heard, moments, moment } = setup();
  strobe.resume();
  moment();
  wake(20);
  heard(50);
  vi.advanceTimersByTime(49);
  assert.equal(moments.length, 0, "not at the earlier wake's time");
  vi.advanceTimersByTime(1);
  assert.equal(moments.length, 1, "at the later wake's time");
});

test("a moment asked before a turn heard runs nothing", () => {
  const { strobe, wake, heard, kicks, moments, moment } = setup();
  strobe.resume();
  moment();
  wake(0);
  heard(NO_WAKE);
  assert.equal(
    moments.length,
    1,
    "the moment asked is the policy's to hand back",
  );
  moment();
  assert.deepEqual(kicks, [4], "and it runs no turn");
  assert.equal(moments.length, 0, "nor asks another");
});

test("a turn the host wants survives a turn heard", () => {
  const { strobe, heard, kicks, moments } = setup();
  strobe.resume();
  moments.shift()?.(performance.now());
  strobe.request();
  heard(NO_WAKE);
  for (const m of moments.splice(0)) m(performance.now());
  assert.deepEqual(kicks, [4, 4], "the host's turn runs at the new moment");
});
