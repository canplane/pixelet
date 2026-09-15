// @xpute/core/sched/tick.ts

/**
 * A guest's scheduler: the middle of the scheduler's three levels.
 *
 * The guest knows little of what is outside it. The central scheduler
 * strobes it with a grant (sched/quantum.ts); in that turn the guest takes
 * what waits on its submission ring, runs its own tasks here, cooperatively,
 * and leaves what goes back on its completion ring. The grant says about how
 * long the turn should take, not a deadline: nothing here is real time. The
 * tick sees the grant and the clock, and nothing of the host.
 *
 * One tick a turn, its phases in a fixed order against one budget. The order
 * *is* the priority:
 *
 *   interaction   what the hand is doing. Not budgeted, and must stay O(1):
 *                 the budget clock is already running, so anything spent
 *                 here comes off what the phases below receive.
 *   visible       what is on screen now.
 *   content       what is about to be on screen.
 *   cosmetic      what only looks better for running.
 *   report        the guest's own record of the turn, after everything
 *                 else, so it sees everything.
 *
 * Within a phase, steps run in the order they registered.
 *
 * **The budget is the grant's, and the guest picks which.** The slice while
 * the view moves; the batch quantum once it has held still for `settle_ms`.
 * The interaction phase says whether it moved (`view_moving`) and the budget
 * is decided after it, so the turn a gesture starts on is already sized for
 * motion.
 *
 * **The content phase is guaranteed a share.** The visible phase may not take
 * `content_share` of the budget: phases run in priority order on one budget,
 * and without the reserve content got only the forward-progress floor while
 * a load kept the visible phase busy.
 *
 * What this does not do: preempt. A step still runs to completion. Steps are
 * small by construction instead.
 *
 * The step table is the tick's own and fixed at its capacity: a guest
 * registers its steps when it starts, and a step past the table is refused
 * there rather than grown into.
 */

import type { f64, u32 } from "../kit/word.ts";
import { now } from "../sys/clock.ts";
import { type PassStats, run_under_budget } from "./frame_budget.ts";
import type { Grant } from "./quantum.ts";

export type Phase = u32;
export const INTERACTION: Phase = 0;
export const VISIBLE: Phase = 1;
export const CONTENT: Phase = 2;
export const COSMETIC: Phase = 3;
export const REPORT: Phase = 4;
export const PHASES = ["interaction", "visible", "content", "cosmetic", "report"] as const;

/** Steps a step may unregister in one call. */
const OFFS_MAX = 8;

/** The guest's numbers. */
export type TickPolicy = {
  /** How long the view must hold still before the batch quantum applies. */
  settle_ms: f64;
  /** How far a pass may run past what is left, so a step that costs real
   * time and reports nothing done cannot walk the whole list —
   * frame_budget.ts's hard ceiling, as slack over the pool. */
  hard_slack_ms: f64;
  /** The share of the budget held back from the visible phase for content. */
  content_share: f64;
};

/**
 * One turn's allowance, shared by every phase.
 *
 * `run` is `run_under_budget` with the numbers filled in from what is left,
 * so a caller states what it wants to do and never what it may spend.
 */
export class FrameBudget {
  /** Held back from whoever asks now, for the phases after it. The tick sets
   * it before the visible phase and clears it before content. */
  reserve_ms = 0;

  /** `t0` is the tick's start, not this call's: the interaction phase runs
   * before the budget is sized, and what it spent comes off the budget. */
  constructor(readonly total_ms: f64, readonly t0: f64, readonly hard_slack_ms: f64) {}

  remaining(): f64 {
    return Math.max(0, this.t0 + this.total_ms - this.reserve_ms - now());
  }

  run<T>(items: Iterable<T>, step: (item: T) => boolean, max_steps?: u32): PassStats {
    const remaining = this.remaining();
    return run_under_budget(items, step, { budget_ms: remaining, hard_budget_ms: remaining + this.hard_slack_ms, max_steps });
  }
}

/** What a step may ask of the turn, and the guest's own state it runs over. */
export class TickContext<S> {
  /** Decided after the interaction phase; steps there must not read it. */
  budget: FrameBudget;
  /** Why the budget is the size it is — both set with it, for the report. */
  frame_ms: f64;
  settled = false;
  /** Wall time each phase has taken so far this tick, filled in as phases
   * complete — so the report phase can read everything before it. */
  readonly phase_ms = [0, 0, 0, 0, 0];
  invalidated = false;
  moved = false;
  readonly offs: u32[] = [];

  constructor(readonly state: S, readonly delta: f64, budget: FrameBudget, frame_ms: f64) {
    this.budget = budget;
    this.frame_ms = frame_ms;
  }

  /** Asks for a turn after this one: the guest is strobed only when
   * something asked, so a step that changes something over time asks for
   * the next. */
  invalidate(): void {
    this.invalidated = true;
  }

  /** Called by an interaction step whose view is still moving. */
  view_moving(): void {
    this.moved = true;
  }

  /** Unregisters a step from inside the tick, before the next step runs. */
  off_tick(id: u32): void {
    if (this.offs.length >= OFFS_MAX) throw new Error(`a step unregistered more than ${OFFS_MAX} steps at once`);
    this.offs.push(id);
  }
}

export type TickStep<S> = (ctx: TickContext<S>) => void;

/** Wall time per step, cumulative until reset. The phase totals can say a
 * phase is heavy and not which of its steps is; this can. Two registrations
 * under one name in one phase add into one row. */
export type StepStats = { phase: Phase; name: string; ms: f64; count: u32; max_ms: f64 };

/** What a turn spent: what the guest reports back to the central scheduler. */
export type TickReport = {
  budget_ms: f64;
  /** The display's frame interval the grant was sized from. */
  frame_ms: f64;
  /** The view had held still for `settle_ms` — the budget is the batch one. */
  settled: boolean;
  phase_ms: f64[];
  /** The whole tick, interaction through report. */
  total_ms: f64;
  /** A turn after this one was asked for. */
  invalidated: boolean;
};

type Registered<S> = { id: u32; phase: Phase; step: TickStep<S>; name: string };

export class Tick<S> {
  private readonly registry: Registered<S>[] = [];
  private next_id = 1;
  private readonly stats: StepStats[] = [];
  private step_timing = false;
  private moved_at = -Infinity;
  private last_report: TickReport = { budget_ms: 0, frame_ms: 0, settled: false, phase_ms: [0, 0, 0, 0, 0], total_ms: 0, invalidated: false };

  constructor(private readonly policy: TickPolicy, private readonly capacity: u32) {}

  /** Registers a step into a phase. Returns the id `off_tick` takes. `name`
   * is what the step's time is reported under. */
  on_tick(phase: Phase, step: TickStep<S>, name: string): u32 {
    if (this.registry.length >= this.capacity) throw new Error(`the tick's ${this.capacity} steps are taken`);
    const id = this.next_id++;
    this.registry.push({ id, phase, step, name });
    return id;
  }

  /** Unregisters a step outside a tick; inside one, a step asks its context. */
  off_tick(id: u32): void {
    this.remove(id);
  }

  /** Whether steps are timed one by one. */
  set_step_timing(on: boolean): void {
    this.step_timing = on;
  }

  step_stats(): readonly StepStats[] {
    return this.stats;
  }

  step_stats_reset(): void {
    this.stats.length = 0;
  }

  /** What the previous turn spent. */
  last(): TickReport {
    return this.last_report;
  }

  private remove(id: u32): number {
    const at = this.registry.findIndex((r) => r.id === id);
    if (at >= 0) this.registry.splice(at, 1);
    return at;
  }

  /** A step's time into its row; a table with no row left drops it. */
  private record(phase: Phase, name: string, ms: f64): void {
    let stat = this.stats.find((s) => s.phase === phase && s.name === name);
    if (!stat) {
      if (this.stats.length >= this.capacity) return;
      this.stats.push(stat = { phase, name, ms: 0, count: 0, max_ms: 0 });
    }
    stat.ms += ms;
    stat.count++;
    stat.max_ms = Math.max(stat.max_ms, ms);
  }

  private run_phase(phase: Phase, ctx: TickContext<S>, t0: f64): void {
    const start = now();
    // Index loop rather than for-of: a step may unregister itself or a
    // sibling, and a snapshot would run something that just asked not to be.
    for (let i = 0; i < this.registry.length;) {
      const r = this.registry[i++];
      if (r.phase !== phase) continue;
      if (this.step_timing) {
        const s0 = now();
        r.step(ctx);
        this.record(r.phase, r.name, now() - s0);
      } else {
        r.step(ctx);
      }
      if (ctx.moved) {
        ctx.moved = false;
        this.moved_at = t0;
      }
      for (const id of ctx.offs) {
        const at = this.remove(id);
        if (at >= 0 && at < i) i--;
      }
      ctx.offs.length = 0;
    }
    ctx.phase_ms[phase] = now() - start;
  }

  /** The guest's turn, on the grant it was strobed with. */
  run(state: S, delta: f64, grant: Grant): TickReport {
    const t0 = now();
    const slack = this.policy.hard_slack_ms;
    // Sized properly once the interaction phase has said whether the view moved.
    const ctx = new TickContext(state, delta, new FrameBudget(0, t0, slack), grant.frame_ms);

    this.run_phase(INTERACTION, ctx, t0);
    ctx.settled = t0 - this.moved_at >= this.policy.settle_ms;
    ctx.budget = new FrameBudget(ctx.settled ? grant.batch_ms : grant.slice_ms, t0, slack);
    ctx.budget.reserve_ms = ctx.budget.total_ms * this.policy.content_share;
    this.run_phase(VISIBLE, ctx, t0);
    ctx.budget.reserve_ms = 0;
    for (const phase of [CONTENT, COSMETIC, REPORT]) this.run_phase(phase, ctx, t0);

    this.last_report = {
      budget_ms: ctx.budget.total_ms,
      frame_ms: ctx.frame_ms,
      settled: ctx.settled,
      phase_ms: [...ctx.phase_ms],
      total_ms: now() - t0,
      invalidated: ctx.invalidated,
    };
    return this.last_report;
  }
}
