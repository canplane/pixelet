// @xpute/host/sched/quantum.ts

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
 * so the frame interval is a whole multiple of the refresh — 60 Hz on a 120 Hz
 * display — and never half of one late. The divisor is stepped as Swappy
 * steps it: up at once where a turn's gap ran a refresh past its interval, a
 * missed vsync; down after `FRAME_WINDOW` interactive turns in a row that
 * missed none, and back up where that proves too soon.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";

export const FRAME_WINDOW = 64;
const MAX_FRAME_GAP_MS = 50;
/** The most refreshes a turn is paced over: 30 Hz on a 120 Hz display. */
const MAX_DIVISOR = 4;

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
  /** The paced interval: the refresh times the divisor. */
  frame_ms: f64;
  interactive: boolean;
}

export class Quantum {
  private readonly gaps = new Float64Array(FRAME_WINDOW).fill(1000 / 60);
  private readonly sorted = new Float64Array(FRAME_WINDOW);
  private gap_at = 0;
  private prev_interactive = false;
  private overrun_ms = 0;
  private input_at = -Infinity;
  private asked_next = false;
  private divisor = 1;
  /** Interactive turns in a row that missed no vsync at this divisor. */
  private held = 0;
  /** The divisor a step down left, which a miss goes back to. */
  private above = MAX_DIVISOR;

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

  /** The interval turns are paced at: a whole number of refreshes. */
  interval(): f64 {
    return this.divisor * this.refresh();
  }

  /** `wake_ms`: 0 for the next frame, NO_WAKE for none. */
  observe(delta_s: f64, turn_ms: f64, grant: Grant, wake_ms: f64): void {
    const gap_ms = delta_s * 1000;
    const interactive = grant.interactive;
    if (interactive && this.prev_interactive && gap_ms <= MAX_FRAME_GAP_MS) {
      const refresh = this.refresh();
      if (gap_ms > grant.frame_ms + refresh / 2) {
        // A vsync missed: paced over one more refresh, at once.
        this.above = Math.min(MAX_DIVISOR, this.divisor + 1);
        this.divisor = this.above;
        this.held = 0;
      } else if (++this.held >= FRAME_WINDOW && this.divisor > 1 && this.divisor <= this.above) {
        // A window held: one refresh fewer, kept while it holds.
        this.above = this.divisor;
        this.divisor -= 1;
        this.held = 0;
      }
    }
    this.prev_interactive = interactive;
    this.overrun_ms = Math.max(0, turn_ms - grant.quota_ms);
    this.asked_next = wake_ms === 0;
  }

  grant(now: f64): Grant {
    const frame = this.interval();
    const p = this.policy;
    const interactive = now - this.input_at < p.settle_ms || this.asked_next;
    const frames = interactive ? 1 : p.batch_frames;
    return { quota_ms: Math.max(0, frame * frames * (1 - p.margin_share) - this.overrun_ms), frame_ms: frame, interactive };
  }
}
