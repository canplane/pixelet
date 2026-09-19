// @xpute/runtime/clock.ts

/**
 * The host's clock, in milliseconds: `performance.now()` unless something
 * installs another — a test that has to hold time still.
 */

import type { f64 } from "@xpute/core/abi/word.ts";

let clock: () => f64 = () => performance.now();

/** Installs the clock. */
export function set_clock(next: () => f64): void {
  clock = next;
}

/** The host's time, in milliseconds. */
export function now(): f64 {
  return clock();
}
