// @xpute/host/sched/strobe.ts

/**
 * When a guest's turn is rung: only when the host (`request`) or the guest's
 * last falling edge asked for one, at a moment the scheduling policy it was
 * booted with gives and admits (policy.ts); a moment it does not admit passes
 * to the next. A guest on a thread of its own (host.ts `Guest.thread`) is
 * kicked with the turn's quota instead of rung, and the policy is handed
 * what it ran as the doorbell hears it (`Doorbell.heard`).
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Doorbell } from "../ipc/doorbell.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

/** HTML's timer delay is a signed 32-bit count of ms; past it a timer fires at once. */
const TIMER_REACH_MS = 2 ** 31 - 1;

export class Strobe<G extends Grant = Grant> {
  /** A moment is asked of the policy and not yet given. */
  private asked = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private timer_at = Infinity;
  private last = -1;
  private running = false;
  /** The last grant a guest on its own thread was kicked with. */
  private granted: G | null = null;
  private readonly consider = (now: f64): void => this.turn(now);

  private readonly doorbell: Doorbell;
  private readonly policy: SchedulingPolicy<G>;
  private readonly submit: (grant: G, delta_s: f64) => void;

  /** `submit` fills the rings before the doorbell rings. */
  constructor(
    doorbell: Doorbell,
    policy: SchedulingPolicy<G>,
    submit: (grant: G, delta_s: f64) => void,
  ) {
    this.doorbell = doorbell;
    this.policy = policy;
    this.submit = submit;
    doorbell.wake = (wake_ms) => this.wake(wake_ms);
    doorbell.turned = (turn_ms, wake_ms) => {
      if (this.granted) this.policy.observe(turn_ms, this.granted, wake_ms);
    };
  }

  /** Turns run from now on; nothing where they already run or the guest stopped. */
  resume(): void {
    if (this.running || this.doorbell.terminated) return;
    this.running = true;
    this.last = performance.now();
    this.request();
  }

  /** No turn runs until `resume`; nothing where none runs. */
  pause(): void {
    if (!this.running) return;
    this.running = false;
    clearTimeout(this.timer);
    this.timer_at = Infinity;
  }

  /** A moment asked of the policy; none while paused or once the guest
   * stopped. */
  request(): void {
    if (this.asked || !this.running || this.doorbell.terminated) return;
    this.asked = true;
    this.policy.next(this.consider);
  }

  private wake(wake_ms: f64): void {
    if (!this.running) return;
    if (wake_ms === 0) return this.request();
    if (wake_ms < 0) return;
    const wait_ms = Math.min(wake_ms, TIMER_REACH_MS);
    const at = performance.now() + wait_ms;
    if (at >= this.timer_at) return;
    clearTimeout(this.timer);
    this.timer_at = at;
    this.timer = setTimeout(() => {
      this.timer_at = Infinity;
      this.request();
    }, wait_ms);
  }

  private turn(now: f64): void {
    this.asked = false;
    if (!this.running || this.doorbell.terminated) return;
    if (!this.policy.admit(now)) return this.request();
    // Unclamped: how long a gap still counts as motion is the guest's call,
    // and a clamp here is where every turn worth reporting would land.
    const delta_s = Math.max(0, (now - this.last) / 1000);
    this.last = now;
    clearTimeout(this.timer);
    this.timer_at = Infinity;
    const grant = this.policy.grant(performance.now());
    // What `submit` sends goes with this turn, so it asks for no other, and
    // kicks nothing before the turn's own kick.
    this.asked = true;
    try {
      this.doorbell.batch(() => this.submit(grant, delta_s));
    } finally {
      this.asked = false;
    }
    if (this.doorbell.kick) {
      this.granted = grant;
      this.doorbell.kick(grant.quota_ms);
      return;
    }
    const t0 = performance.now();
    const wake_ms = this.doorbell.ring(grant.quota_ms);
    this.policy.observe(performance.now() - t0, grant, wake_ms);
  }
}
