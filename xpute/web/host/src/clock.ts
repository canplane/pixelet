// @xpute/web-host/clock.ts

/** The clocks the host serves the guest (wit/clocks/monotonic-clock.wit,
 * wit/clocks/system-clock.wit), in nanoseconds as wasi:clocks counts them: the
 * monotonic one on `performance.now()`, its waits on the event loop's own
 * timers, and the system clock on `Date.now()`. */

import { round } from "@xpute/kit/math/scalar.ts";
import { ERRNO_ECANCELED } from "@xpute/kit/status/errno.spec.ts";
import { XputeError } from "@xpute/kit/status/error.ts";

import * as monotonic_clock from "@xpute/link/abi/clocks/monotonic_clock.ts";
import * as system_clock from "@xpute/link/abi/clocks/system_clock.ts";
import type { Link } from "@xpute/link";

const NS_PER_MS = 1e6;

export function serve_clock(link: Link): void {
  monotonic_clock.mount(link, {
    now: () => BigInt(round(performance.now() * NS_PER_MS)),
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
  system_clock.mount(link, {
    now() {
      const ms = Date.now();
      return {
        seconds: BigInt(Math.floor(ms / 1000)),
        nanoseconds: (ms % 1000) * NS_PER_MS,
      };
    },
  });
}
