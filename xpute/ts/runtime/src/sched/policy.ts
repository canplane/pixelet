// @xpute/runtime/sched/policy.ts

/**
 * The scheduler asks this of the scheduling policy it was booted with: when a
 * turn it was asked for may run, and when each turn must end. The scheduler
 * is the mechanism, and the policy decides. The scheduler rings only when the
 * runtime or the core's last falling edge asked for a turn, keeps the core's
 * `wake_ms`, and hands the policy what each turn took. A page's policy paces
 * turns to the display (frame_pacing.ts); a runtime with no display may run a
 * turn as soon as it is asked for, or on a timer of its own.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";

/** One turn is given its `deadline`, the time on the runtime's clock
 * (`performance.now()`) by which it ends, and whatever else the policy says
 * about the turn. */
export interface Grant {
  deadline: f64;
}

export interface SchedulingPolicy<G extends Grant = Grant> {
  /** Calls `consider` at the next moment a turn might run: an animation
   * frame on a page, the next pass of a loop elsewhere. */
  next(consider: (now: f64) => void): void;
  /** This says whether a turn runs at `now`; one that does not is considered
   * again at the next moment. */
  admit(now: f64): boolean;
  /** This gives the grant of the turn about to run. */
  grant(now: f64): G;
  /** Hears how long the turn took and what its falling edge asked for. */
  observe(turn_ms: f64, grant: G, wake_ms: f64): void;
  /** Hears an input at `now`; a policy may treat the turns after it as
   * interactive (frame_pacing.ts). */
  input?(now: f64): void;
}
