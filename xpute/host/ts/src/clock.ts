// @xpute/host/clock.ts

/** The clocks the host implements for the guest
 * (wit/clocks/monotonic-clock.wit, wit/clocks/system-clock.wit), in nanoseconds
 * as wasi:clocks counts them: the monotonic one on `performance.now()`, its
 * waits on the event loop's own timers and a wait of 0 on a posted message,
 * and the system clock on `Date.now()`.
 */

import type { u64 } from "@xpute/kit/abi/word.ts";
import { round } from "@xpute/kit/math/scalar.ts";
import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

import * as monotonic_clock from "./abi/clocks/monotonic_clock.ts";
import * as system_clock from "./abi/clocks/system_clock.ts";
import type { Side } from "@xpute/link/link.ts";

const NS_PER_MS = 1e6;

/** The monotonic clock's mark of `ms`, a time on `performance.now()`'s line
 * in its scope, as a platform's timestamp of an event lies on it. */
export function mark(ms: number): u64 {
  return BigInt(round(ms * NS_PER_MS));
}

/** Calls `f` at the event loop's next pass, by a posted message, and returns
 * what stops it. A timer set inside a timer's task nests, and a nested timer
 * is clamped to 4 ms however short it is asked; a message is not. */
function next_pass(f: () => void): () => void {
  const c = new MessageChannel();
  c.port1.onmessage = () => {
    c.port1.close();
    f();
  };
  c.port2.postMessage(null);
  return () => c.port1.close();
}

/** Calls `f` once `ns` nanoseconds have passed, and returns what stops it. */
function after(ns: u64, f: () => void): () => void {
  const timer = setTimeout(f, Number(ns) / NS_PER_MS);
  return () => clearTimeout(timer);
}

export function mount_clock(side: Side): void {
  monotonic_clock.mount(side, {
    now: () => mark(performance.now()),
    // A wait of 0 answers at the event loop's next pass, as the WIT says; a
    // wait its caller cancels stops what would answer it and answers
    // ECANCELED.
    wait_for: (how_long, signal) =>
      new Promise((done, fail) => {
        const stop = how_long === 0n ? next_pass(done) : after(how_long, done);
        signal.addEventListener("abort", () => {
          stop();
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
