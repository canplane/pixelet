// @xpute/host/sched/strobe.ts

/**
 * When a guest's turn is rung: only when the host (`request`, `sent`) or the
 * guest's last falling edge asked for one, at a moment the scheduling policy
 * it was booted with gives and admits (policy.ts); a moment it does not admit
 * passes to the next.
 *
 * A guest on a thread of its own (host.ts `Guest.thread`) runs its turns as
 * kicks and its own wake time ask, so the strobe keeps no timer for it and
 * asks no turn for what the host sent, which the doorbell kicked. It paces
 * only what the guest's thread cannot see: the policy's moments, a display's
 * frames, each a frame's grant submitted and kicked with its quota, asked for
 * where the guest asked for the next at once (`Doorbell.ack`'s 0) or the
 * host did (`request`). The policy is
 * handed what the thread ran as the doorbell hears it.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Doorbell } from "../ipc/doorbell.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

/** HTML's timer delay is a signed 32-bit count of ms; past it a timer fires at once. */
const TIMER_REACH_MS = 2 ** 31 - 1;

export class Strobe<G extends Grant = Grant> {
  /** A moment is asked of the policy and not yet given. */
  private _asked = false;
  private _timer: ReturnType<typeof setTimeout> | undefined;
  private _timer_at = Infinity;
  private _last = -1;
  private _running = false;
  /** The last grant a guest on its own thread was kicked with. */
  private _granted: G | null = null;
  private readonly _consider = (now: f64): void => this._turn(now);

  private readonly _doorbell: Doorbell;
  private readonly _policy: SchedulingPolicy<G>;
  private readonly _submit: (grant: G, delta_s: f64) => void;

  /** `submit` fills the rings before the doorbell rings. */
  constructor(
    doorbell: Doorbell,
    policy: SchedulingPolicy<G>,
    submit: (grant: G, delta_s: f64) => void,
  ) {
    this._doorbell = doorbell;
    this._policy = policy;
    this._submit = submit;
    doorbell.wake = (wake_ms) => this._wake(wake_ms);
    doorbell.on_turn = (turn_ms, wake_ms) => {
      if (this._granted) this._policy.observe(turn_ms, this._granted, wake_ms);
    };
  }

  /** Turns run from now on; nothing where they already run or the guest stopped. */
  resume(): void {
    if (this._running || this._doorbell.terminated) return;
    this._running = true;
    this._last = performance.now();
    this.request();
  }

  /** No turn runs until `resume`; nothing where none runs. */
  pause(): void {
    if (!this._running) return;
    this._running = false;
    clearTimeout(this._timer);
    this._timer_at = Infinity;
  }

  /** Wakes the guest for what the host submitted: asks a moment for a guest
   * that runs only when rung; nothing for one on its own thread, which the
   * doorbell kicked as it submitted. */
  kick(): void {
    if (!this._doorbell.kick) this.request();
  }

  /** A moment asked of the policy; none while paused or once the guest
   * stopped. */
  request(): void {
    if (this._asked || !this._running || this._doorbell.terminated) return;
    this._asked = true;
    this._policy.next(this._consider);
  }

  private _wake(wake_ms: f64): void {
    if (!this._running) return;
    if (wake_ms === 0) return this.request();
    if (wake_ms < 0 || this._doorbell.kick) return;
    const wait_ms = Math.min(wake_ms, TIMER_REACH_MS);
    const at = performance.now() + wait_ms;
    if (at >= this._timer_at) return;
    clearTimeout(this._timer);
    this._timer_at = at;
    this._timer = setTimeout(() => {
      this._timer_at = Infinity;
      this.request();
    }, wait_ms);
  }

  private _turn(now: f64): void {
    this._asked = false;
    if (!this._running || this._doorbell.terminated) return;
    if (!this._policy.admit(now)) return this.request();
    // Unclamped: how long a gap still counts as motion is the guest's call,
    // and a clamp here is where every turn worth reporting would land.
    const delta_s = Math.max(0, (now - this._last) / 1000);
    this._last = now;
    clearTimeout(this._timer);
    this._timer_at = Infinity;
    const grant = this._policy.grant(performance.now());
    // What `submit` sends goes with this turn, so it asks for no other, and
    // kicks nothing before the turn's own kick.
    this._asked = true;
    try {
      this._doorbell.batch(() => this._submit(grant, delta_s));
    } finally {
      this._asked = false;
    }
    if (this._doorbell.kick) {
      this._granted = grant;
      this._doorbell.kick(grant.quota_ms);
      return;
    }
    const t0 = performance.now();
    const wake_ms = this._doorbell.ring(grant.quota_ms);
    this._policy.observe(performance.now() - t0, grant, wake_ms);
  }
}
