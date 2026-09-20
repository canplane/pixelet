// @xpute/runtime/sched/quantum.ts

/**
 * The central scheduler's grant: the outermost of the scheduler's three
 * levels.
 *
 * The central scheduler runs guests the way an OS runs processes: it owns
 * the clock and the resources, and the turn alternates like a clock's two
 * edges. On the rising edge the host hands the guest one quota, the time it
 * may take; the guest runs its own tasks cooperatively (sched/tick.ts) and,
 * on the falling edge, hands the turn back with when it wants the next one.
 * The host knows nothing of what runs inside. Memory is granted once
 * (mem/section.ts, mem/transfer.ts), and I/O by credits (sched/io.ts).
 *
 * **One quota, sized from what the host sees.** Nothing is preempted, so a
 * turn may run past its quota by the step that noticed; what it overran
 * comes off its own next quota, down to nothing, and the host's share of
 * the frame is left as it was (a constant bandwidth server's rule). Whether
 * the turn is interactive is the host's call, as an OS scheduler's is, from
 * behaviour rather than from the guest's inside: input within `settle_ms`,
 * or a guest that asked for the very next edge. An interactive turn gets
 * what the frame leaves after the margin; otherwise that of `batch_frames`
 * frames, past one on purpose — with nothing moving, a frame that takes two
 * vsyncs shows the same image twice.
 *
 * **No number here is a measured cost.** The quota is a share of the frame
 * the display sets; how much work fits in it is the guest's to find out by
 * yielding, and an overrun corrects itself through the next quota. The floor
 * is not a number of milliseconds either: a pass always takes its first step
 * (sched/frame_budget.ts), so a quota of nothing still makes progress.
 *
 * **The frame interval is measured, not assumed.** `1000 / 60` is wrong on
 * any faster display: a 5.8 ms frame was once sized for 16.7, and a storm
 * spent more than a whole frame scheduling. It is a low percentile of recent
 * gaps, not their mean — the mean includes the frames the guest itself made
 * long, which would feed an overrun back into a bigger slice. Only gaps
 * between two interactive turns count, and the percentile is the 25th: a
 * held-still turn runs long on purpose, and the host answers a late frame by
 * firing the next one almost at once — gaps that once read as a "2.1 ms
 * display" on a 170 Hz screen and pinned every moving frame to the floor.
 * Gaps past MAX_FRAME_GAP_MS are not frames at all: an idle guest asks for
 * none, and the gap before the next can be seconds.
 *
 * Every number that decides how much is the guest's policy (QuantumPolicy),
 * declared to the central scheduler, not chosen here.
 */

import type { f64 } from "@xpute/core/abi/word.ts";

export const FRAME_WINDOW = 64;
const MAX_FRAME_GAP_MS = 50;

/** A falling edge that asks for no next edge: the guest waits on something
 * only the host can bring, an input or an arrival. */
export const NO_WAKE: f64 = -1;

export interface QuantumPolicy {
  /** What is left of the frame unspent, as a share of it: the host's own
   * work, its collector, and the turns the estimate is wrong about. */
  margin_share: f64;
  /** The frames a turn that is not interactive may take. */
  batch_frames: f64;
  /** How long after the last input a turn is still interactive. */
  settle_ms: f64;
}

/** The rising edge: how long the guest may take, and why. */
export interface Grant {
  quota_ms: f64;
  /** The display's measured frame interval. */
  frame_ms: f64;
  interactive: boolean;
}

/** A turn as the host saw it, once the falling edge came back. */
export interface Turn {
  /** The gap before the turn, in seconds. */
  delta_s: f64;
  /** The whole turn, around the call. */
  turn_ms: f64;
  /** The grant the turn ran on. */
  grant: Grant;
  /** When the guest wants the next edge, in ms from now: 0 the next frame,
   * NO_WAKE none. */
  wake_ms: f64;
}

export class Quantum {
  private readonly gaps = new Float64Array(FRAME_WINDOW).fill(1000 / 60);
  private readonly sorted = new Float64Array(FRAME_WINDOW);
  private gap_at = 0;
  private prev_interactive = false;
  /** What the last turn overran its quota by. */
  private overrun_ms = 0;
  private input_at = -Infinity;
  private asked_next = false;

  constructor(private readonly policy: QuantumPolicy) {}

  /** The host saw input at `now`. */
  input(now: f64): void {
    this.input_at = now;
  }

  /** Learns from a turn that finished. */
  observe(turn: Turn): void {
    const gap_ms = turn.delta_s * 1000;
    const interactive = turn.grant.interactive;
    if (interactive && this.prev_interactive && gap_ms > 0 && gap_ms <= MAX_FRAME_GAP_MS) {
      this.gaps[this.gap_at] = gap_ms;
      this.gap_at = (this.gap_at + 1) % FRAME_WINDOW;
    }
    this.prev_interactive = interactive;
    this.overrun_ms = Math.max(0, turn.turn_ms - turn.grant.quota_ms);
    this.asked_next = turn.wake_ms === 0;
  }

  /** The rising edge's grant at `now`. */
  grant(now: f64): Grant {
    this.sorted.set(this.gaps);
    this.sorted.sort();
    const frame = this.sorted[FRAME_WINDOW / 4];
    const p = this.policy;
    const interactive = now - this.input_at < p.settle_ms || this.asked_next;
    const frames = interactive ? 1 : p.batch_frames;
    return { quota_ms: Math.max(0, frame * frames * (1 - p.margin_share) - this.overrun_ms), frame_ms: frame, interactive };
  }
}
