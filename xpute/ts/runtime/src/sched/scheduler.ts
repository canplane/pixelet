// @xpute/runtime/sched/scheduler.ts

/**
 * Decides when a core's turn is rung. A turn is rung only when the runtime
 * (`wake`) or the core's last falling edge asked for one, and only at a
 * moment that the scheduling policy the scheduler was booted with gives and
 * admits (policy.ts). A moment the policy does not admit passes the turn on
 * to the next moment.
 *
 * A core on a thread of its own (runtime.ts `Core.thread`) runs its turns as
 * kicks ask and keeps no timer, so this scheduler is its only scheduler, as
 * it is for a page's core. Each wake the core asks for, at once or in `n` ms,
 * as the doorbell hears it (`Doorbell.turns`, `ring`), and each wake the
 * runtime asks for (`wake`), waits for a moment the policy gives and admits.
 * At that moment the turn's grant is submitted and the core is kicked with
 * its deadline. What the runtime sends is such a wake on either core, and
 * the granted turn applies it.
 *
 * Every turn is heard (`Doorbell.on_turn`): each turn the thread ran, whoever
 * kicked it, and each turn rung. A turn heard drops the timer and every
 * moment asked for an earlier wake: a moment the policy hands back from
 * before the turn runs nothing, and only a turn the runtime still wants is
 * asked for again, so the scheduler never holds a request it no longer means.
 * The policy is told what the thread ran, as the doorbell hears it.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Doorbell } from "../ipc/doorbell.ts";
import type { Grant, SchedulingPolicy } from "./policy.ts";

/** HTML's timer delay is a signed 32-bit count of ms; past it a timer fires at once. */
const TIMER_REACH_MS = 2 ** 31 - 1;

export class Scheduler<G extends Grant = Grant> {
  /** Counts the turns heard. A moment asked for before a turn is no longer
   * meant once that turn is heard, because the wake the turn asks for is told
   * after it. */
  private _gen = 0;
  /** This holds, for each moment asked of the policy and not yet given, the
   * generation it was asked under, in the order the policy gives the
   * moments. */
  private readonly _asked_gens: number[] = [];
  /** The runtime asked for a turn that has not run yet. Unlike a moment, this
   * outlives a turn heard. */
  private _runtime_wants = false;
  /** Set while `submit` runs: what it sends goes with the turn being
   * submitted, so `wake` asks for no other. */
  private _batching = false;
  private _timer: ReturnType<typeof setTimeout> | undefined;
  private _timer_at = Infinity;
  private _last = -1;
  private _running = false;
  /** This is the last grant a core on its own thread was kicked with. */
  private _granted: G | null = null;
  private readonly _consider = (now: f64): void => this._turn(now);

  private readonly _doorbell: Doorbell;
  private readonly _policy: SchedulingPolicy<G>;
  private readonly _submit: (grant: G, delta_s: f64) => void;

  /** `submit` fills the rings for a granted turn before the doorbell
   * rings. */
  constructor(
    doorbell: Doorbell,
    policy: SchedulingPolicy<G>,
    submit: (grant: G, delta_s: f64) => void,
  ) {
    this._doorbell = doorbell;
    this._policy = policy;
    this._submit = submit;
    doorbell.on_wake = (wake_ms) => this._wake_in(wake_ms);
    // A turn heard, whoever kicked or rang it, is followed by the wake it asks
    // for (`Doorbell.turns`, `ring`). So the timer and the moments asked for
    // an earlier wake are dropped, and only a turn the runtime still wants is
    // asked for again.
    doorbell.on_turn = (turn_ms, wake_ms) => {
      this._gen++;
      this._drop_timer();
      if (this._granted) this._policy.observe(turn_ms, this._granted, wake_ms);
      if (this._runtime_wants) this._ask();
    };
  }

  /** Lets turns run from now on. It does nothing if they already run or the
   * core has stopped. */
  resume(): void {
    if (this._running || this._doorbell.terminated) return;
    this._running = true;
    this._last = performance.now();
    this.wake();
  }

  /** Stops turns until `resume`. It does nothing if none run. */
  pause(): void {
    if (!this._running) return;
    this._running = false;
    this._drop_timer();
  }

  /** Asks for a turn the runtime wants, at a moment asked of the policy. It
   * asks nothing while paused or once the core has stopped, nor for what goes
   * with the turn being submitted. */
  wake(): void {
    if (this._batching) return;
    this._runtime_wants = true;
    this._ask();
  }

  /** Asks the policy for a moment, at most once in a generation. */
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
    // The gap is not clamped. How long a gap still counts as motion is for the
    // core to decide, and a clamp here would pin every turn worth reporting to
    // the clamp's value.
    const delta_s = Math.max(0, (now - this._last) / 1000);
    this._last = now;
    this._drop_timer();
    const grant = this._policy.grant(performance.now());
    // What `submit` sends goes with this turn, so it asks for no other turn.
    this._batching = true;
    try {
      this._submit(grant, delta_s);
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

function left_ms(grant: Grant): f64 {
  return Math.max(0, grant.deadline - performance.now());
}
