// @xpute/hypervisor/sched/quantum.ts

/**
 * The host's grant for a guest's turn. A turn may overrun its quota, and the
 * overrun comes off its next one. A turn is interactive after input within
 * `settle_ms` or when the guest asked for the next edge, and then gets what the
 * frame leaves after the margin; otherwise `batch_frames` frames.
 *
 * The display's refresh is a low percentile of the gaps between the
 * animation frames the host is handed (`tick`), every one, run or skipped: not
 * 1000 / 60 and not the mean, as the mean feeds an overrun back into a larger
 * slice, and the near-immediate frame after a late one once read a 170 Hz
 * display as 2.1 ms. Gaps past MAX_FRAME_GAP_MS are idle, not frames.
 *
 * A turn runs on every `divisor`-th refresh (Android Swappy's swap interval),
 * so the frame interval is a whole multiple of the refresh and never half of
 * one late: the most refreshes that fit `PACED_MAX_MS`, 60 Hz, and at least
 * one — 60 Hz on a 120 Hz display, 72 on a 144, 90 on a 90, and the refresh
 * itself where a browser caps it below 60. The divisor is the display's, not
 * the load's: what a frame costs is the guest's to fit, so one controller
 * answers it.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";

export const FRAME_WINDOW = 64;
const MAX_FRAME_GAP_MS = 50;
/** The longest interval turns are paced at: 60 Hz. */
const PACED_MAX_MS = 1000 / 60;

/** The guest waits on something only the host brings: an input or an arrival. */
export const NO_WAKE: f64 = -1;

export interface QuantumPolicy {
  /** The frame share left unspent, for the host's own work and estimate error. */
  margin_share: f64;
  batch_frames: f64;
  settle_ms: f64;
}

export interface Grant {
  quota_ms: f64;
  /** The display's refresh, and the interval turns are paced at: the refresh times the divisor. */
  refresh_ms: f64;
  paced_ms: f64;
  interactive: boolean;
}

export class Quantum {
  private readonly gaps = new Float64Array(FRAME_WINDOW).fill(1000 / 60);
  private readonly sorted = new Float64Array(FRAME_WINDOW);
  private gap_at = 0;
  private overrun_ms = 0;
  private input_at = -Infinity;
  private asked_next = false;

  constructor(private readonly policy: QuantumPolicy) {}

  input(now: f64): void {
    this.input_at = now;
  }

  /** The gap between two animation frames the host was handed, in turn. */
  tick(gap_ms: f64): void {
    if (gap_ms > 0 && gap_ms <= MAX_FRAME_GAP_MS) {
      this.gaps[this.gap_at] = gap_ms;
      this.gap_at = (this.gap_at + 1) % FRAME_WINDOW;
    }
  }

  /** The display's refresh, from the ticks. */
  refresh(): f64 {
    this.sorted.set(this.gaps);
    this.sorted.sort();
    return this.sorted[FRAME_WINDOW / 4];
  }

  /**
   * The interval turns are paced at: the most whole refreshes within
   * `PACED_MAX_MS`, an eighth of a refresh allowed for the spread of its
   * measure, so a 120 Hz display read at 8.34 ms still paces at two; one at
   * least.
   */
  paced(refresh_ms: f64): f64 {
    const divisor = Math.max(1, Math.floor((PACED_MAX_MS + refresh_ms / 8) / refresh_ms));
    return divisor * refresh_ms;
  }

  /** `wake_ms`: 0 for the next frame, NO_WAKE for none. */
  observe(turn_ms: f64, grant: Grant, wake_ms: f64): void {
    this.overrun_ms = Math.max(0, turn_ms - grant.quota_ms);
    this.asked_next = wake_ms === 0;
  }

  grant(now: f64): Grant {
    const refresh_ms = this.refresh();
    const paced_ms = this.paced(refresh_ms);
    const p = this.policy;
    const interactive = now - this.input_at < p.settle_ms || this.asked_next;
    const frames = interactive ? 1 : p.batch_frames;
    return { quota_ms: Math.max(0, paced_ms * frames * (1 - p.margin_share) - this.overrun_ms), refresh_ms, paced_ms, interactive };
  }
}
