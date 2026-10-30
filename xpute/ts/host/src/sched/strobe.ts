// @xpute/host/sched/strobe.ts

/**
 * When a guest's turn is rung: only when the host (`request`) or the guest's
 * last falling edge asked for one, at a moment the scheduling policy it was
 * booted with gives and admits (policy.ts); a moment it does not admit passes
 * to the next.
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
  private readonly consider = (now: f64): void => this.turn(now);

  /** `submit` fills the rings before the doorbell rings. */
  constructor(
    private readonly doorbell: Doorbell,
    private readonly policy: SchedulingPolicy<G>,
    private readonly submit: (grant: G, delta_s: f64) => void,
  ) {
    doorbell.wake = (wake_ms) => this.wake(wake_ms);
  }

  start(): void {
    this.running = true;
    this.last = performance.now();
    this.request();
  }

  stop(): void {
    this.running = false;
    clearTimeout(this.timer);
    this.timer_at = Infinity;
  }

  request(): void {
    if (this.asked || !this.running) return;
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
    if (!this.running) return;
    if (!this.policy.admit(now)) return this.request();
    // Unclamped: how long a gap still counts as motion is the guest's call,
    // and a clamp here is where every turn worth reporting would land.
    const delta_s = Math.max(0, (now - this.last) / 1000);
    this.last = now;
    clearTimeout(this.timer);
    this.timer_at = Infinity;
    const grant = this.policy.grant(performance.now());
    this.submit(grant, delta_s);
    const t0 = performance.now();
    const wake_ms = this.doorbell.ring(grant.quota_ms);
    this.policy.observe(performance.now() - t0, grant, wake_ms);
  }
}
