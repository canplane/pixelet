// @xpute/runtime/sched/frame_pacing.ts

/**
 * A scheduling policy (policy.ts) for a runtime that shares its thread with a
 * display's compositor, as a page does: turns run on animation frames, a
 * whole number of refreshes apart, and each gets what its frames leave. None
 * of it is xpute's mechanism; the program that composes a page injects it,
 * with its numbers, when the runtime boots.
 *
 * A turn's deadline is its frame's, as Android's Choreographer hands a frame
 * its deadline: the frame's start and what its frames leave after the
 * margin. A turn rung late into its frame has that much less; one that ran
 * past its deadline owes the next nothing. A turn
 * is interactive after input within `settle_ms` or when the core asked for the
 * next edge, and then gets what the frame leaves after the margin; otherwise
 * `batch_frames` frames.
 *
 * The display's refresh is a low percentile of the gaps between the
 * animation frames the runtime is handed (`gap`), every one seen, run or
 * skipped: not the mean, which feeds an overrun back into a larger slice and
 * reads the near-immediate frame after a late one as a fast display. Gaps past
 * MAX_FRAME_GAP_MS are idle, not frames. Before the first gap the refresh is
 * taken as one interval at `pace_hz`, so the first turn's deadline is the
 * program's own interval away.
 *
 * A turn runs on every `divisor`-th refresh (Android Swappy's swap interval),
 * the most refreshes that fit one interval at `pace_hz` and at least one, so
 * the frame interval is a whole multiple of the refresh: at 60, every second
 * refresh of a 120 Hz display and of a 144. The divisor is the display's, not
 * the load's.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

export const FRAME_WINDOW = 64;
const MAX_FRAME_GAP_MS = 50;

/** The program's numbers for its pacing. */
export interface FramePacingConfig {
  /** The slowest rate turns are paced at. */
  pace_hz: f64;
  /** The frame share left unspent, for the runtime's own work and estimate error. */
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
  private readonly gaps = new Float64Array(FRAME_WINDOW);
  private readonly sorted = new Float64Array(FRAME_WINDOW);
  private gap_at = 0;
  /** How many of `gaps` are frames seen, up to the window. */
  private seen = 0;
  /** The display's refresh as of the last gap seen: a percentile of the
   * window, sorted once a gap and read by every admit and grant after it. */
  private _refresh_ms: f64;
  private input_at = -Infinity;
  private asked_next = false;
  /** When the last animation frame was handed, run or skipped. */
  private frame_at = -1;
  /** When the last turn ran. */
  private last = -Infinity;

  private readonly cfg: FramePacingConfig;
  /** The longest interval turns are paced at. */
  private readonly max_ms: f64;
  private readonly next_frame: (run: (now: f64) => void) => void;

  /** `next_frame` hands over the display's next animation frame. */
  constructor(
    cfg: FramePacingConfig,
    next_frame: (run: (now: f64) => void) => void,
  ) {
    this.cfg = cfg;
    this.max_ms = 1000 / cfg.pace_hz;
    this._refresh_ms = this.max_ms;
    this.next_frame = next_frame;
  }

  next(consider: (now: f64) => void): void {
    this.next_frame(consider);
  }

  /** An input at `now`: the turns after it are interactive for `settle_ms`. */
  input(now: f64): void {
    this.input_at = now;
  }

  /** The gap between two animation frames the runtime was handed, in turn. */
  gap(gap_ms: f64): void {
    if (gap_ms > 0 && gap_ms <= MAX_FRAME_GAP_MS) {
      this.gaps[this.gap_at] = gap_ms;
      this.gap_at = (this.gap_at + 1) % FRAME_WINDOW;
      this.seen = Math.min(this.seen + 1, FRAME_WINDOW);
      const n = this.seen;
      const seen = this.sorted.subarray(0, n);
      seen.set(this.gaps.subarray(0, n));
      seen.sort();
      this._refresh_ms = seen[n >> 2];
    }
  }

  /** The display's refresh, from the gaps seen. */
  refresh(): f64 {
    return this._refresh_ms;
  }

  /**
   * The interval turns are paced at: the most whole refreshes within one
   * interval at `pace_hz`, an eighth of a refresh allowed for the spread of its
   * measure, so a 120 Hz display read at 8.34 ms still paces at two; one at
   * least.
   */
  pace_ms(refresh_ms: f64): f64 {
    const divisor = Math.max(
      1,
      Math.floor((this.max_ms + refresh_ms / 8) / refresh_ms),
    );
    return divisor * refresh_ms;
  }

  /** Paced: a frame nearer the last turn than the interval, less half a
   * refresh for the frame's own jitter, is skipped, and the next considered. */
  admit(now: f64): boolean {
    if (this.frame_at >= 0) this.gap(now - this.frame_at);
    this.frame_at = now;
    const refresh_ms = this.refresh();
    if (now - this.last < this.pace_ms(refresh_ms) - refresh_ms / 2)
      return false;
    this.last = now;
    return true;
  }

  /** `wake_ms`: 0 for the next turn, NO_WAKE (wake.ts) for none. */
  observe(_turn_ms: f64, _grant: FrameGrant, wake_ms: f64): void {
    this.asked_next = wake_ms === 0;
  }

  grant(now: f64): FrameGrant {
    const refresh_ms = this.refresh();
    const paced_ms = this.pace_ms(refresh_ms);
    const c = this.cfg;
    const interactive = now - this.input_at < c.settle_ms || this.asked_next;
    const frames = interactive ? 1 : c.batch_frames;
    // From the frame's start, so a turn rung late into it has the less.
    const start = this.frame_at >= 0 ? this.frame_at : now;
    return {
      deadline: start + paced_ms * frames * (1 - c.margin_share),
      refresh_ms,
      paced_ms,
      interactive,
    };
  }
}
