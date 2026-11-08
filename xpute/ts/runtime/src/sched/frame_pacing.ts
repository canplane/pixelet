// @xpute/runtime/sched/frame_pacing.ts

/**
 * A scheduling policy (policy.ts) for a runtime that shares its thread with a
 * display's compositor, as a page does. Turns run on animation frames, a
 * whole number of refreshes apart, and each turn gets the time its frames
 * leave. None of this is xpute's mechanism: the program that composes a page
 * injects this policy, with its own numbers, when the runtime boots.
 *
 * A turn's deadline is its frame's, as Android's Choreographer hands a frame
 * its deadline: the frame's start plus what its frames leave after the
 * margin. A turn rung late into its frame has that much less time, and a
 * turn that ran past its deadline owes the next turn nothing. A turn is
 * interactive within `settle_ms` after an input, or when the core asked for
 * the next turn at once. An interactive turn gets one frame, and any other
 * turn `batch_frames` frames, less the margin either way.
 *
 * The display's refresh is a low percentile of the gaps between the
 * animation frames the runtime is handed (`gap`), counting every frame seen,
 * whether a turn ran on it or not. It is not the mean, which would feed an
 * overrun back into a larger slice and would read the near-immediate frame
 * after a late one as a fast display. A gap longer than MAX_FRAME_GAP_MS is
 * idle time, not a frame. Before the first gap the refresh is taken to be one
 * interval at `pace_hz`, so the first turn's deadline is the program's own
 * interval away.
 *
 * A turn runs on every `divisor`-th refresh (Android Swappy's swap interval),
 * so the interval between turns is a whole multiple of the refresh. The
 * divisor is the most refreshes that fit one interval at `pace_hz`, and at
 * least one: at 60 Hz, that is every second refresh of a 120 Hz display and
 * of a 144 Hz one. When the program last asked for a longer least interval
 * (`pace`), the divisor is instead the fewest refreshes that reach it. The
 * program judges its own load; an input hurries no turn past what the
 * program asked.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

export const FRAME_WINDOW = 64;
const MAX_FRAME_GAP_MS = 50;

/** The program's numbers for its pacing. */
export interface FramePacingConfig {
  /** The rate turns are paced at, unless the program asks for a slower one
   * (`pace`). */
  pace_hz: f64;
  /** The share of each frame left unspent, for the runtime's own work and
   * for error in the estimate. */
  margin_share: f64;
  batch_frames: f64;
  settle_ms: f64;
}

export interface FrameGrant extends Grant {
  /** `refresh_ms` is the display's refresh, and `paced_ms` the interval
   * turns are paced at, which is the refresh times the divisor. */
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
  /** The display's refresh as of the last gap seen. It is a percentile of
   * the window, sorted once for each gap and read by every admit and grant
   * after it. */
  private _refresh_ms: f64;
  private input_at = -Infinity;
  private asked_next = false;
  /** When the last animation frame was handed, run or skipped. */
  private frame_at = -1;
  /** When the last turn ran. */
  private last = -Infinity;

  private readonly cfg: FramePacingConfig;
  /** The interval `pace_hz` paces at. */
  private readonly max_ms: f64;
  /** The least interval the program asked between turns, 0 for none. */
  private _asked_ms: f64 = 0;
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

  /** Records the gap between two successive animation frames the runtime was
   * handed. */
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

  /** Sets the least interval between turns from now on; 0 sets none. */
  pace(interval_ms: f64): void {
    this._asked_ms = interval_ms;
  }

  /** The display's refresh, from the gaps seen. */
  refresh(): f64 {
    return this._refresh_ms;
  }

  /**
   * The interval turns are paced at: the most whole refreshes within one
   * interval at `pace_hz`, or the fewest that reach the interval the program
   * asked for, whichever is more, and at least one. Each bound allows an
   * eighth of a refresh for the spread of the refresh's measure, so a 120 Hz
   * display measured at 8.34 ms still paces 60 Hz at two refreshes.
   */
  pace_ms(refresh_ms: f64): f64 {
    const spread = refresh_ms / 8;
    const divisor = Math.max(
      1,
      Math.floor((this.max_ms + spread) / refresh_ms),
      Math.ceil((this._asked_ms - spread) / refresh_ms),
    );
    return divisor * refresh_ms;
  }

  /** Skips a frame that comes sooner after the last turn than the paced
   * interval, less half a refresh for the frame's own jitter; the next frame
   * is then considered. */
  admit(now: f64): boolean {
    if (this.frame_at >= 0) this.gap(now - this.frame_at);
    this.frame_at = now;
    const refresh_ms = this.refresh();
    if (now - this.last < this.pace_ms(refresh_ms) - refresh_ms / 2)
      return false;
    this.last = now;
    return true;
  }

  /** `wake_ms` is 0 when the core wants the next turn at once, and NO_WAKE
   * (wake.ts) when it wants none. */
  observe(_turn_ms: f64, _grant: FrameGrant, wake_ms: f64): void {
    this.asked_next = wake_ms === 0;
  }

  grant(now: f64): FrameGrant {
    const refresh_ms = this.refresh();
    const paced_ms = this.pace_ms(refresh_ms);
    const c = this.cfg;
    const interactive = now - this.input_at < c.settle_ms || this.asked_next;
    const frames = interactive ? 1 : c.batch_frames;
    // The deadline counts from the frame's start, so a turn rung late into
    // its frame has that much less time.
    const start = this.frame_at >= 0 ? this.frame_at : now;
    return {
      deadline: start + paced_ms * frames * (1 - c.margin_share),
      refresh_ms,
      paced_ms,
      interactive,
    };
  }
}
