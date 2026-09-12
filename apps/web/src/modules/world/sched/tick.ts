// @pxl/web/modules/world/sched/tick.ts

/**
 * The frame, as one tick.
 *
 * What this replaces was ten `useFrame` subscribers, each running in the
 * order React happened to mount them, each doing its own work with no idea
 * how much the others had already spent. Two of them carried a budget —
 * chunk_stage's recheck and commit passes — and those two budgets were
 * independent, so a frame could legally spend both in full before anything
 * else ran. The rest ran unbudgeted, including one step measured at 22 ms.
 *
 * Here there is one tick, called from one `useFrame`, and it runs its phases
 * in a fixed order against one budget. The order *is* the priority:
 *
 *   interaction   camera, frame rebasing, the puck — what the hand is doing.
 *                 Not budgeted, and must stay O(1): the budget clock is
 *                 already running, so anything spent here comes off what
 *                 the phases below receive.
 *   visible       LOD recheck and commit — what is on screen now.
 *   content       index sweeps, placement — what is about to be on screen.
 *   cosmetic      fades, environment lerps, decor and billboard packing.
 *   report        the ledger, after everything else, so it sees everything.
 *
 * Within a phase, steps run in the order they registered, which is mount
 * order — the same order the subscribers had before, kept on purpose so this
 * changes *when* things run relative to the budget and not relative to each
 * other.
 *
 * **The budget is derived, not chosen.** `frame_budget.ts` hands a pass a
 * number of milliseconds and the number used to be a constant. This reads the
 * ledger's own record of what the rest of the frame costs — the main-thread
 * render time outside this tick — and hands the phases what is left of the
 * frame after that, so a heavier scene gets less scheduling time and a
 * lighter one gets more. That is the feedback control fetch_scheduler.ts's
 * header says it does not have. It is clamped: never so little that nothing
 * makes progress, never so much that a GC pause has no room.
 *
 * What this does not do: preempt. A step still runs to completion, and a
 * step that can take 15 ms still takes 15 ms. That is the next thing to fix
 * and it is fixed by making steps small by construction, not by anything
 * here — see the discussion this comes from.
 */

import { useLayoutEffect, useRef } from "react";
import { type RootState, useFrame } from "@react-three/fiber";

import { type BudgetOpts, type PassStats, run_under_budget } from "./frame_budget.ts";
import { ledger_last_frame } from "./ledger.ts";

export type Phase = "interaction" | "visible" | "content" | "cosmetic" | "report";
export const PHASES: readonly Phase[] = ["interaction", "visible", "content", "cosmetic", "report"];

export type TickContext = {
  state: RootState;
  delta: number;
  budget: FrameBudget;
  /** Wall time each phase has taken so far this tick, filled in as phases
   * complete — so the report phase can read everything before it. */
  phase_ms: Record<Phase, number>;
};
export type TickStep = (ctx: TickContext) => void;

/**
 * The display's frame interval, measured rather than assumed.
 *
 * It was `1000 / 60`, which is wrong on any faster display: the ledger read a
 * p50 frame of 5.8 ms while the tick was sized for 16.7, so a storm could
 * spend more than a whole frame scheduling and drop every other one.
 *
 * Taken as a low percentile of recent frame gaps, not their mean. The mean
 * includes the frames this tick itself made long, so sizing the budget from
 * it would feed an overrun back into a bigger budget. The fast edge of the
 * distribution is the refresh interval: with vsync, gaps come in multiples
 * of it, and the smallest common one is it. Gaps past MAX_FRAME_GAP_MS are
 * not frames at all — under frameloop="demand" an idle canvas draws nothing,
 * and the gap before the next frame can be seconds.
 */
const FRAME_WINDOW = 64;
const MAX_FRAME_GAP_MS = 50;
const frame_gaps = new Float64Array(FRAME_WINDOW).fill(1000 / 60);
const $gaps = new Float64Array(FRAME_WINDOW);
let frame_gap_at = 0;

function observe_frame_gap(delta_s: number): void {
  const ms = delta_s * 1000;
  if (!(ms > 0) || ms > MAX_FRAME_GAP_MS) return;
  frame_gaps[frame_gap_at] = ms;
  frame_gap_at = (frame_gap_at + 1) % FRAME_WINDOW;
}

function frame_ms(): number {
  $gaps.set(frame_gaps);
  $gaps.sort();
  return $gaps[Math.floor(FRAME_WINDOW * 0.1)];
}
/** Left unspent on purpose, for the collector and for the frames the
 * estimate below is wrong about — a share of the frame rather than a
 * constant, so a fast display does not lose most of its frame to it. */
const MARGIN_SHARE = 0.15;
/** The clamp. The floor is what keeps a starved frame making progress; the
 * ceiling is what keeps a cheap scene from letting one tick grow into most
 * of a frame and leaving nothing for a spike. */
const MIN_BUDGET_MS = 2;
const MAX_BUDGET_MS = 8;
/** How far a pass may run past its share, so a step that costs real time and
 * reports nothing done cannot walk the whole list — frame_budget.ts's own
 * hard ceiling, expressed as slack over the pool rather than as a second
 * constant per pass. */
const HARD_SLACK_MS = 4;
/** Smoothing on the render estimate. One frame is noise; ten is a trend. */
const RENDER_EMA = 0.1;

/**
 * One frame's allowance, shared by every phase.
 *
 * `run` is `run_under_budget` with the numbers filled in from what is left,
 * so a caller states what it wants to do and never what it may spend.
 */
export class FrameBudget {
  readonly t0: number;
  readonly total_ms: number;

  constructor(total_ms: number) {
    this.t0 = performance.now();
    this.total_ms = total_ms;
  }

  remaining(): number {
    return Math.max(0, this.t0 + this.total_ms - performance.now());
  }

  run<T>(items: readonly T[], step: (item: T) => boolean, opts: Pick<BudgetOpts, "max_steps"> = {}): PassStats {
    const remaining = this.remaining();
    return run_under_budget(items, step, {
      budget_ms: remaining,
      hard_budget_ms: remaining + HARD_SLACK_MS,
      max_steps: opts.max_steps,
    });
  }
}

type Registered = { phase: Phase; step: TickStep };
const registry: Registered[] = [];

/** Registers a step into a phase. Returns the unregister. */
export function on_tick(phase: Phase, step: TickStep): () => void {
  const entry: Registered = { phase, step };
  registry.push(entry);
  return () => {
    const i = registry.indexOf(entry);
    if (i >= 0) registry.splice(i, 1);
  };
}

/**
 * The React face of `on_tick`: the step always sees the latest closure, and
 * registration happens in a layout effect so it lands in mount order — the
 * same order `useFrame` used to subscribe in.
 */
export function useTick(phase: Phase, step: TickStep): void {
  const latest = useRef(step);
  latest.current = step;
  useLayoutEffect(() => on_tick(phase, (ctx) => latest.current(ctx)), [phase]);
}

export type TickReport = {
  budget_ms: number;
  /** Wall time each phase took this frame. */
  phase_ms: Record<Phase, number>;
  /** The whole tick, interaction through report. */
  total_ms: number;
};

let render_ema_ms = 4;
let last_report: TickReport = { budget_ms: 0, phase_ms: { interaction: 0, visible: 0, content: 0, cosmetic: 0, report: 0 }, total_ms: 0 };

/** What the previous tick spent, per phase. For the ledger. */
export function tick_last(): TickReport {
  return last_report;
}

/** Main-thread render cost outside this tick, learned from the ledger: the
 * whole rAF task minus the scheduler's own pass. */
function observe_render_cost(): void {
  const last = ledger_last_frame();
  if (!last) return;
  const outside = Math.max(0, last.task_ms - last.sched_ms);
  render_ema_ms += (outside - render_ema_ms) * RENDER_EMA;
}

function derive_budget_ms(): number {
  const frame = frame_ms();
  return Math.min(MAX_BUDGET_MS, Math.max(MIN_BUDGET_MS, frame - frame * MARGIN_SHARE - render_ema_ms));
}

export function run_tick(state: RootState, delta: number): TickReport {
  observe_frame_gap(delta);
  observe_render_cost();
  const budget = new FrameBudget(derive_budget_ms());
  const phase_ms: Record<Phase, number> = { interaction: 0, visible: 0, content: 0, cosmetic: 0, report: 0 };
  const ctx: TickContext = { state, delta, budget, phase_ms };

  for (const phase of PHASES) {
    const t0 = performance.now();
    // Index loop rather than for-of: a step may unregister itself or a
    // sibling, and a snapshot would run something that just asked not to be.
    for (let i = 0; i < registry.length; i++) {
      const entry = registry[i];
      if (entry.phase === phase) entry.step(ctx);
    }
    phase_ms[phase] = performance.now() - t0;
  }

  last_report = { budget_ms: budget.total_ms, phase_ms, total_ms: performance.now() - budget.t0 };
  return last_report;
}

/** The one `useFrame` in the world. Mount once, inside the Canvas, before
 * anything that registers a step. */
export function TickDriver() {
  useFrame((state, delta) => {
    run_tick(state, delta);
  });
  return null;
}
