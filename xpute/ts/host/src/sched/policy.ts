// @xpute/host/sched/policy.ts

/**
 * What the strobe asks of the scheduling policy it was booted with: when a
 * turn it was asked for may run, and how much each gets. The strobe is the
 * mechanism: it rings only when the host or the guest's last falling edge
 * asked, keeps the guest's `wake_ms`, and hands the policy what each turn
 * took. A page's policy paces turns to the display (frame_pacing.ts); a host
 * with no display may run one as soon as it is asked, or on a timer of its own.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";

/** What one turn is given: its quota, and whatever else the policy says of it. */
export interface Grant {
  quota_ms: f64;
}

export interface SchedulingPolicy<G extends Grant = Grant> {
  /** Calls `consider` at the next moment a turn might run: an animation
   * frame on a page, the next pass of a loop elsewhere. */
  next(consider: (now: f64) => void): void;
  /** Whether a turn runs at `now`; one that does not is considered again at
   * the next moment. */
  admit(now: f64): boolean;
  /** The grant of the turn about to run. */
  grant(now: f64): G;
  /** What the turn took and what its falling edge asked for. */
  observe(turn_ms: f64, grant: G, wake_ms: f64): void;
}
