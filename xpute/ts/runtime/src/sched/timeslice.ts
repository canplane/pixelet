// @xpute/runtime/sched/timeslice.ts

/**
 * A scheduling policy (policy.ts) for a runtime that has no display to pace
 * to and shares its thread with the core. A turn asked for runs at the event
 * loop's next pass and ends by a deadline one slice away, as SCHED_RR's
 * timeslice ends a turn on a shared CPU.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

export class Timeslice implements SchedulingPolicy {
  private readonly slice_ms: f64;

  constructor(slice_ms: f64) {
    this.slice_ms = slice_ms;
  }

  next(consider: (now: f64) => void): void {
    setTimeout(() => consider(performance.now()), 0);
  }

  admit(): boolean {
    return true;
  }

  grant(now: f64): Grant {
    return { deadline: now + this.slice_ms };
  }

  observe(): void {}
}
