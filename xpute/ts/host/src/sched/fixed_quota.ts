// @xpute/host/sched/fixed_quota.ts

/**
 * A scheduling policy (policy.ts) for a host with no display to pace to: a
 * turn asked for runs at the event loop's next pass, each given the same
 * quota.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

export class FixedQuota implements SchedulingPolicy {
  constructor(private readonly quota_ms: f64) {}

  next(consider: (now: f64) => void): void {
    setTimeout(() => consider(performance.now()), 0);
  }

  admit(): boolean {
    return true;
  }

  grant(): Grant {
    return { quota_ms: this.quota_ms };
  }

  observe(): void {}
}
