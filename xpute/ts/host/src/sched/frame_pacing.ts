// @xpute/host/sched/frame_pacing.ts

/**
 * A scheduling policy (policy.ts) for a host that shares its thread with a
 * display's compositor, as a page does: turns run on animation frames, a
 * whole number of refreshes apart, and each gets what its frames leave. None
 * of it is xpute's mechanism; the program that composes a page injects it,
 * with its numbers, when the host boots.
 *
 * A turn may overrun its quota, and the overrun comes off its next one. A turn
 * is interactive after input within `settle_ms` or when the guest asked for the
 * next edge, and then gets what the frame leaves after the margin; otherwise
 * `batch_frames` frames.
 *
 * The display's refresh is a low percentile of the gaps between the
 * animation frames the host is handed (`gap`), every one, run or skipped: not
 * 1000 / 60 and not the mean, which feeds an overrun back into a larger slice
 * and reads the near-immediate frame after a late one as a fast display. Gaps
 * past MAX_FRAME_GAP_MS are idle, not frames.
 *
 * A turn runs on every `divisor`-th refresh (Android Swappy's swap interval),
 * the most refreshes that fit `PACED_MAX_MS` and at least one, so the frame
 * interval is a whole multiple of the refresh: 60 Hz on a 120 Hz display, 72
 * on a 144. The divisor is the display's, not the load's.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

export const FRAME_WINDOW = 64;
const MAX_FRAME_GAP_MS = 50;
/** The slowest rate turns are paced at. */
const PACED_MIN_HZ = 60;
/** The longest interval turns are paced at. */
const PACED_MAX_MS = 1000 / PACED_MIN_HZ;
/** The display's refresh until gaps are seen: taken as 60 Hz, whatever the pacing. */
const FIRST_REFRESH_MS = 1000 / 60;

/** The program's numbers for its pacing. */
export interface FramePacingConfig {
  /** The frame share left unspent, for the host's own work and estimate error. */
  margin_share: f64;
  batch_frames: f64;
  settle_ms: f64;
}

export interface FrameGrant extends Grant {
  /** The display's refresh, and the interval turns are paced at: the refresh times the divisor. */
  refresh_ms: f64;
  paced_ms: f64;
  interactive: boolean;
}

export class FramePacing implements SchedulingPolicy<FrameGrant> {
  private readonly gaps = new Float64Array(FRAME_WINDOW).fill(FIRST_REFRESH_MS);
  private readonly sorted = new Float64Array(FRAME_WINDOW);
  private gap_at = 0;
  private overrun_ms = 0;
  private input_at = -Infinity;
  private asked_next = false;
  /** When the last animation frame was handed, run or skipped. */
  private frame_at = -1;
  /** When the last turn ran. */
  private last = -Infinity;

  /** `next_frame` hands over the display's next animation frame. */
  constructor(
    private readonly cfg: FramePacingConfig,
    private readonly next_frame: (run: (now: f64) => void) => void,
  ) {}

  next(consider: (now: f64) => void): void {
    this.next_frame(consider);
  }

  /** An input at `now`: the turns after it are interactive for `settle_ms`. */
  input(now: f64): void {
    this.input_at = now;
  }

  /** The gap between two animation frames the host was handed, in turn. */
  gap(gap_ms: f64): void {
    if (gap_ms > 0 && gap_ms <= MAX_FRAME_GAP_MS) {
      this.gaps[this.gap_at] = gap_ms;
      this.gap_at = (this.gap_at + 1) % FRAME_WINDOW;
    }
  }

  /** The display's refresh, from the gaps. */
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
    const divisor = Math.max(
      1,
      Math.floor((PACED_MAX_MS + refresh_ms / 8) / refresh_ms),
    );
    return divisor * refresh_ms;
  }

  /** Paced: a frame nearer the last turn than the interval, less half a
   * refresh for the frame's own jitter, is skipped, and the next considered. */
  admit(now: f64): boolean {
    if (this.frame_at >= 0) this.gap(now - this.frame_at);
    this.frame_at = now;
    const refresh_ms = this.refresh();
    if (now - this.last < this.paced(refresh_ms) - refresh_ms / 2) return false;
    this.last = now;
    return true;
  }

  /** `wake_ms`: 0 for the next turn, NO_WAKE (wake.ts) for none. */
  observe(turn_ms: f64, grant: FrameGrant, wake_ms: f64): void {
    this.overrun_ms = Math.max(0, turn_ms - grant.quota_ms);
    this.asked_next = wake_ms === 0;
  }

  grant(now: f64): FrameGrant {
    const refresh_ms = this.refresh();
    const paced_ms = this.paced(refresh_ms);
    const c = this.cfg;
    const interactive = now - this.input_at < c.settle_ms || this.asked_next;
    const frames = interactive ? 1 : c.batch_frames;
    return {
      quota_ms: Math.max(
        0,
        paced_ms * frames * (1 - c.margin_share) - this.overrun_ms,
      ),
      refresh_ms,
      paced_ms,
      interactive,
    };
  }
}
