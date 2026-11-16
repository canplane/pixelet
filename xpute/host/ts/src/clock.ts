// @xpute/host/clock.ts

/** The clocks the host implements for the guest
 * (wit/clocks/monotonic-clock.wit, wit/clocks/system-clock.wit), in nanoseconds
 * as wasi:clocks counts them: the monotonic one on `performance.now()`, its
 * waits on the event loop's own timers, and the system clock on `Date.now()`.
 */

import type { u64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

import * as monotonic_clock from "./abi/clocks/monotonic_clock.ts";
import * as system_clock from "./abi/clocks/system_clock.ts";
import type { Side } from "@xpute/link";

const NS_PER_MS = 1e6;

/** The monotonic clock's mark of `ms`, a time on `performance.now()`'s line
 * in its scope, as a platform's timestamp of an event lies on it. */
export function mark(ms: number): u64 {
  return BigInt(round(ms * NS_PER_MS));
}

export function mount_clock(side: Side): void {
  monotonic_clock.mount(side, {
    now: () => mark(performance.now()),
    // A wait its caller cancels stops its timer and answers ECANCELED.
    wait_for: (how_long, signal) =>
      new Promise((done, fail) => {
        const timer = setTimeout(done, Number(how_long) / NS_PER_MS);
        signal.addEventListener("abort", () => {
          clearTimeout(timer);
          fail(new XputeError(ERRNO_ECANCELED));
        });
      }),
  });
  system_clock.mount(side, {
    now() {
      const ms = Date.now();
      return {
        seconds: BigInt(Math.floor(ms / 1000)),
        nanoseconds: (ms % 1000) * NS_PER_MS,
      };
    },
  });
}
