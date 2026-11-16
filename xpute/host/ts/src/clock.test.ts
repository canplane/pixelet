// @xpute/host/clock.test.ts

import { afterEach, test, vi } from "vitest";
import assert from "node:assert/strict";

import {
  ERRNO_ECANCELED,
  ERRNO_EINPROGRESS,
} from "@xpute/kit/status/errno.spec.ts";
import { HANDLE, WAIT_FOR } from "./abi/clocks/monotonic_clock.ts";
import type { LinkObject, Side, Tag } from "@xpute/link";
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
  return args;
};

test("a canceled wait stops its timer and answers ECANCELED, and a wait answers once its time has passed", async () => {
  vi.useFakeTimers();
  const { clock, answers, stops } = mounted();
  assert.equal(
    clock.call(WAIT_FOR, 1, wait_args(1_000_000_000n)),
    ERRNO_EINPROGRESS,
  );
  assert.equal(
    clock.call(WAIT_FOR, 2, wait_args(1_000_000n)),
    ERRNO_EINPROGRESS,
  );
  stops.get(1)!();
  await vi.advanceTimersByTimeAsync(2_000);
  assert.deepEqual(answers, [
    [1, ERRNO_ECANCELED],
    [2, 0],
  ]);
});
