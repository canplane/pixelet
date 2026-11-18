// @xpute/host/clock.test.ts

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import {
  ERRNO_ECANCELED,
  ERRNO_EINPROGRESS,
} from "@xpute/kit/status/errno.spec.ts";
import { HANDLE, WAIT_FOR } from "./abi/clocks/monotonic_clock.ts";
import { Lift, Lower } from "@xpute/link/cabi.ts";
import { type LinkObject, type Side, type Tag } from "@xpute/link/link.ts";
import { mount_clock } from "./clock.ts";

afterEach(() => {
  vi.useRealTimers();
});

/** The clock mounted on a link that keeps what it is given: the monotonic
 * clock, each answer by its tag, and each request's stop. */
function mounted(): {
  clock: LinkObject;
  answers: [Tag, number][];
  stops: Map<Tag, () => void>;
} {
  const named = new Map<number, LinkObject>();
  const answers: [Tag, number][] = [];
  const stops = new Map<Tag, () => void>();
  mount_clock({
    mount_named: (h: number, o: LinkObject) => named.set(h, o),
    task_return: (t: Tag, result: number) => answers.push([t, result]),
    on_cancel: (t: Tag, stop: () => void) => stops.set(t, stop),
  } as unknown as Side);
  return { clock: named.get(HANDLE)!, answers, stops };
}

/** `wait-for`'s args: how long, in nanoseconds. */
const wait_args = (ns: bigint) => {
  const args = new Uint8Array(8);
  new DataView(args.buffer).setBigUint64(0, ns, true);
  return new Lift(args);
};

/** The writer of an answer, which a wait never writes. */
const ret = new Lower(new Uint8Array(0), 0);

/** Waits for the event loop's next pass, as a posted message comes. */
const next_pass = () =>
  new Promise((done) => {
    const c = new MessageChannel();
    c.port1.onmessage = () => {
      c.port1.close();
      done(null);
    };
    c.port2.postMessage(null);
  });

test("a wait of 0 answers at the event loop's next pass with no timer, and a canceled one answers ECANCELED alone", async () => {
  vi.useFakeTimers();
  const { clock, answers, stops } = mounted();
  for (const tag of [1, 2]) {
    assert.equal(
      clock.call(WAIT_FOR, tag, wait_args(0n), ret),
      ERRNO_EINPROGRESS,
    );
  }
  stops.get(2)!();
  await next_pass();
  assert.deepEqual(answers, [
    [2, ERRNO_ECANCELED],
    [1, 0],
  ]);
});

test("a canceled wait stops its timer and answers ECANCELED, and a wait answers once its time has passed", async () => {
  vi.useFakeTimers();
  const { clock, answers, stops } = mounted();
  assert.equal(
    clock.call(WAIT_FOR, 1, wait_args(1_000_000_000n), ret),
    ERRNO_EINPROGRESS,
  );
  assert.equal(
    clock.call(WAIT_FOR, 2, wait_args(1_000_000n), ret),
    ERRNO_EINPROGRESS,
  );
  stops.get(1)!();
  await vi.advanceTimersByTimeAsync(2_000);
  assert.deepEqual(answers, [
    [1, ERRNO_ECANCELED],
    [2, 0],
  ]);
});
