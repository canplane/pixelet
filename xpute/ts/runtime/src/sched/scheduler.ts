// @xpute/runtime/sched/scheduler.ts

/**
 * When a core's turn is rung: only when the runtime (`wake`, `kick`) or the
 * core's last falling edge asked for one, at a moment the scheduling policy
 * it was booted with gives and admits (policy.ts); a moment it does not admit
 * passes to the next.
 *
 * A core on a thread of its own (runtime.ts `Core.thread`) runs its turns as
 * kicks ask and keeps no timer, so the scheduler is its one scheduler as it is
 * a page core's: each wake the core asks, at once or in `n` ms, as the
 * doorbell hears it (`Doorbell.ack`, `ring`), and each the runtime asks
 * (`wake`), waits for the moment the policy gives and admits, where a
 * frame's grant is submitted and kicked with its deadline. It asks no turn for
 * what the runtime sent, which the doorbell kicked. Every turn is heard
 * (`Doorbell.on_turn`), the thread's whoever kicked it and each one rung,
 * and lets go the timer and the moments asked for an earlier wake: a moment
 * the policy hands back from before it runs nothing, and only a turn the
 * runtime still wants is asked again, so the scheduler holds no request it no
 * longer means. The policy is handed what the thread ran as the doorbell
 * hears it.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Doorbell } from "../ipc/doorbell.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

/** HTML's timer delay is a signed 32-bit count of ms; past it a timer fires at once. */
const TIMER_REACH_MS = 2 ** 31 - 1;

export class Scheduler<G extends Grant = Grant> {
  /** Bumped by every turn heard: a moment asked before it is no longer
   * meant, since the wake that turn asks is told after it. */
  private _gen = 0;
  /** The generation each moment asked of the policy and not yet given was
   * asked under, in the order the policy gives them. */
  private readonly _asked_gens: number[] = [];
  /** The runtime asked for a turn that has not run yet: kept past a turn heard. */
  private _runtime_wants = false;
  /** What `submit` sends goes with the turn being submitted. */
  private _batching = false;
  private _timer: ReturnType<typeof setTimeout> | undefined;
  private _timer_at = Infinity;
  private _last = -1;
  private _running = false;
  /** The last grant a core on its own thread was kicked with. */
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
    doorbell.wake = (wake_ms) => this._wake_in(wake_ms);
    // A turn heard, whoever kicked or rang it, ends with the wake it asks,
    // told next (`Doorbell.ack`, `ring`): the timer and the moments asked for
    // an earlier wake go, and only what the runtime still wants is asked again.
    doorbell.on_turn = (turn_ms, wake_ms) => {
      this._gen++;
      this._drop_timer();
      if (this._granted) this._policy.observe(turn_ms, this._granted, wake_ms);
      if (this._runtime_wants) this._ask();
    };
  }

  /** Turns run from now on; nothing where they already run or the core stopped. */
  resume(): void {
    if (this._running || this._doorbell.terminated) return;
    this._running = true;
    this._last = performance.now();
    this.wake();
  }

  /** No turn runs until `resume`; nothing where none runs. */
  pause(): void {
    if (!this._running) return;
    this._running = false;
    this._drop_timer();
  }

  /** Wakes the core for what the runtime submitted: asks a moment for a core
   * that runs only when rung; nothing for one on its own thread, which the
   * doorbell kicked as it submitted. */
  kick(): void {
    if (!this._doorbell.kick) this.wake();
  }

  /** A turn the runtime wants, at a moment asked of the policy; none while
   * paused or once the core stopped, and none for what goes with the turn
   * being submitted. */
  wake(): void {
    if (this._batching) return;
    this._runtime_wants = true;
    this._ask();
  }

  /** A moment asked of the policy under this generation, once. */
  private _ask(): void {
    if (!this._running || this._doorbell.terminated) return;
    if (this._asked_gens.at(-1) === this._gen) return;
    this._asked_gens.push(this._gen);
    this._policy.next(this._consider);
  }

  private _drop_timer(): void {
    clearTimeout(this._timer);
    this._timer_at = Infinity;
  }

  private _wake_in(wake_ms: f64): void {
    if (!this._running) return;
    if (wake_ms === 0) return this._ask();
    if (wake_ms < 0) return;
    const wait_ms = Math.min(wake_ms, TIMER_REACH_MS);
    const at = performance.now() + wait_ms;
    if (at >= this._timer_at) return;
    clearTimeout(this._timer);
    this._timer_at = at;
    this._timer = setTimeout(() => {
      this._timer_at = Infinity;
      this._ask();
    }, wait_ms);
  }

  private _turn(now: f64): void {
    if (this._asked_gens.shift() !== this._gen) return;
    if (!this._running || this._doorbell.terminated) return;
    if (!this._policy.admit(now)) return this._ask();
    this._runtime_wants = false;
    // Unclamped: how long a gap still counts as motion is the core's call,
    // and a clamp here is where every turn worth reporting would land.
    const delta_s = Math.max(0, (now - this._last) / 1000);
    this._last = now;
    this._drop_timer();
    const grant = this._policy.grant(performance.now());
    // What `submit` sends goes with this turn, so it asks for no other, and
    // kicks nothing before the turn's own kick.
    this._batching = true;
    try {
      this._doorbell.batch(() => this._submit(grant, delta_s));
    } finally {
      this._batching = false;
    }
    if (this._doorbell.kick) {
      this._granted = grant;
      this._doorbell.kick(left_ms(grant));
      return;
    }
    const t0 = performance.now();
    const wake_ms = this._doorbell.ring(left_ms(grant));
    this._policy.observe(performance.now() - t0, grant, wake_ms);
  }
}

/** The ms from now to the grant's deadline, 0 where it has passed. */
function left_ms(grant: Grant): f64 {
  return Math.max(0, grant.deadline - performance.now());
}
