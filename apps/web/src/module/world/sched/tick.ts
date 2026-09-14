// @pxl/web/module/world/sched/tick.ts

/**
 * The frame, as one tick.
 *
 * What this replaces was ten per-frame subscribers of three's loop, each running in the
 * order React happened to mount them, each doing its own work with no idea
 * how much the others had already spent. Two of them carried a budget —
 * the chunk stage's recheck and commit passes — and those two budgets were
 * independent, so a frame could legally spend both in full before anything
 * else ran. The rest ran unbudgeted, including one step measured at 22 ms.
 *
 * Here there is one tick, run once per animation frame the world asked
 * for, and it runs its phases in a fixed order against one budget. The order *is* the priority:
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
 * **A still view gets a bigger budget.** The budget exists to keep motion
 * smooth, and when nothing on screen is moving there is no motion to keep
 * smooth: a frame that takes two vsyncs instead of one shows the same image
 * twice. So once the view has held still for SETTLE_MS, the tick is sized
 * for throughput instead, and the backlog a pan left behind drains several
 * times faster. The view steps in the interaction phase say when they moved
 * (`view_moving`), and the budget is decided after that phase, so the frame
 * a gesture starts on is already back on the small budget.
 *
 * What this does not do: preempt. A step still runs to completion. Steps are
 * small by construction instead — the scan and the bake advance a few rows
 * or a few hundred boxes per call (voxel_scan.ts, chunk_mesher.ts).
 */

import { useLayoutEffect, useRef } from "react";

import { post_cfg } from "@/module/world/environment/config.ts";

import { type BudgetOpts, type PassStats, run_under_budget } from "./frame_budget.ts";
import { ledger_last_frame } from "./ledger.ts";
import { bind_tick_request, world_wakers } from "@/module/world/change.ts";

export type Phase = "interaction" | "visible" | "content" | "cosmetic" | "report";
export const PHASES: readonly Phase[] = ["interaction", "visible", "content", "cosmetic", "report"];

/** What a step may ask of the frame: another frame, the viewport, the clock. */
export type TickState = {
  /** Asks for a frame after this one: the tick runs only when something
   * asked, so a step that changes something over time asks for the next. */
  invalidate(): void;
  /** The viewport in CSS pixels, and the device pixels each one is. */
  size: { width: number; height: number };
  viewport: { dpr: number };
  clock: { getElapsedTime(): number };
};

export type TickContext = {
  state: TickState;
  delta: number;
  /** Decided after the interaction phase; steps there must not read it. */
  budget: FrameBudget;
  /** Called by an interaction step whose view is still moving — the camera
   * damping toward its target, the frame rotating or translating. */
  view_moving: () => void;
  /** Why the budget is the size it is — both set with it, for the ledger. */
  frame_ms: number;
  settled: boolean;
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
 * not frames at all — an idle canvas draws nothing,
 * and the gap before the next frame can be seconds.
 *
 * Only gaps between two moving frames count, and the percentile is the
 * 25th rather than the 10th. A still frame runs long on purpose, and the
 * browser answers a late frame by firing the next one almost at once — the
 * ledger read a "2.1 ms display" on a 170 Hz screen from those catch-up gaps,
 * which pinned every moving frame to the budget floor.
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
  return $gaps[Math.floor(FRAME_WINDOW * 0.25)];
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
/** How long the view must hold still before the tick stops guarding
 * smoothness. Long enough that a pause inside one drag does not count. */
const SETTLE_MS = 150;
/** The still-view budget. Past a 60 Hz frame on purpose: at 170 Hz this is
 * two or three vsyncs of an unchanging image, and at 60 Hz one frame plus
 * render — either way nothing the eye can follow is waiting on it. */
const SETTLED_BUDGET_MS = 12;
/** How far a pass may run past its share, so a step that costs real time and
 * reports nothing done cannot walk the whole list — frame_budget.ts's own
 * hard ceiling, expressed as slack over the pool rather than as a second
 * constant per pass. */
export const HARD_SLACK_MS = 4;
/** Smoothing on the render estimate. One frame is noise; ten is a trend. */
const RENDER_EMA = 0.1;
/**
 * The share of the budget the visible phase may not take: what content gets
 * even in a storm. Phases run in priority order on one budget, and without
 * this content got only the forward-progress floor — one unit a frame —
 * while a load kept the visible phase busy. A placement of a few dozen
 * units then took 25 frames and 3.3 s on a phone, and an index rebuild 49
 * frames (2026-09-13). A fixed share, not a feedback loop: the scheduler
 * still knows one number.
 */
const CONTENT_SHARE = 0.2;

/**
 * One frame's allowance, shared by every phase.
 *
 * `run` is `run_under_budget` with the numbers filled in from what is left,
 * so a caller states what it wants to do and never what it may spend.
 */
export class FrameBudget {
  readonly t0: number;
  readonly total_ms: number;
  /** Held back from whoever asks now, for the phases after it. The tick
   * sets it before the visible phase and clears it before content. */
  reserve_ms = 0;

  /** `t0` is the tick's start, not this call's: the interaction phase runs
   * before the budget is sized, and what it spent comes off the budget. */
  constructor(total_ms: number, t0: number = performance.now()) {
    this.t0 = t0;
    this.total_ms = total_ms;
  }

  remaining(): number {
    return Math.max(0, this.t0 + this.total_ms - this.reserve_ms - performance.now());
  }

  run<T>(items: Iterable<T>, step: (item: T) => boolean, opts: Pick<BudgetOpts, "max_steps"> = {}): PassStats {
    const remaining = this.remaining();
    return run_under_budget(items, step, {
      budget_ms: remaining,
      hard_budget_ms: remaining + HARD_SLACK_MS,
      max_steps: opts.max_steps,
    });
  }
}

type Registered = { phase: Phase; step: TickStep; name: string };
const registry: Registered[] = [];

/**
 * Per-step wall time, dev only, keyed `phase/name`, cumulative since the
 * ledger window began (log.ts resets it). The phase totals said the
 * interaction phase cost a phone 7 ms a frame and could not say which of
 * its five steps did; this can. Two registrations under one name (the two
 * companion walkers) add into one row.
 */
export type StepStats = { name: string; ms: number; count: number; max_ms: number };
// Tested under deno, where import.meta.env does not exist.
const STEP_TIMING: boolean = import.meta.env?.DEV ?? false;
const step_stats = new Map<string, StepStats>();

export function tick_step_stats(): StepStats[] {
  return [...step_stats.values()];
}
export function tick_step_stats_reset(): void {
  step_stats.clear();
}

/** Registers a step into a phase. Returns the unregister. `name` is what
 * the ledger reports the step's time under. */
export function on_tick(phase: Phase, step: TickStep, name = "step"): () => void {
  const entry: Registered = { phase, step, name };
  registry.push(entry);
  return () => {
    const i = registry.indexOf(entry);
    if (i >= 0) registry.splice(i, 1);
  };
}

/**
 * The React face of `on_tick`: the step always sees the latest closure, and
 * registration happens in a layout effect so it lands in mount order — the
 * same order three's loop used to subscribe in.
 */
export function useTick(phase: Phase, step: TickStep, name = "step"): void {
  const latest = useRef(step);
  latest.current = step;
  useLayoutEffect(() => on_tick(phase, (ctx) => latest.current(ctx), name), [phase, name]);
}

export type TickReport = {
  budget_ms: number;
  /** The display's measured frame interval when the budget was sized. */
  frame_ms: number;
  /** The view had held still for SETTLE_MS — the budget is the large one. */
  settled: boolean;
  /** Wall time each phase took this frame. */
  phase_ms: Record<Phase, number>;
  /** The whole tick, interaction through report. */
  total_ms: number;
};

let render_ema_ms = 4;
let moved_at = -Infinity;
let prev_settled = true;
let last_report: TickReport = {
  budget_ms: 0,
  frame_ms: 0,
  settled: false,
  phase_ms: { interaction: 0, visible: 0, content: 0, cosmetic: 0, report: 0 },
  total_ms: 0,
};

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

function derive_budget_ms(frame: number, settled: boolean): number {
  if (settled) return SETTLED_BUDGET_MS;
  return Math.min(MAX_BUDGET_MS, Math.max(MIN_BUDGET_MS, frame - frame * MARGIN_SHARE - render_ema_ms));
}

function run_phase(phase: Phase, ctx: TickContext): void {
  const t0 = performance.now();
  // Index loop rather than for-of: a step may unregister itself or a
  // sibling, and a snapshot would run something that just asked not to be.
  for (let i = 0; i < registry.length; i++) {
    const entry = registry[i];
    if (entry.phase !== phase) continue;
    if (!STEP_TIMING) {
      entry.step(ctx);
      continue;
    }
    const s0 = performance.now();
    entry.step(ctx);
    const ms = performance.now() - s0;
    const key = `${phase}/${entry.name}`;
    let stat = step_stats.get(key);
    if (!stat) step_stats.set(key, stat = { name: key, ms: 0, count: 0, max_ms: 0 });
    stat.ms += ms;
    stat.count++;
    if (ms > stat.max_ms) stat.max_ms = ms;
  }
  ctx.phase_ms[phase] = performance.now() - t0;
}

export function run_tick(state: TickState, delta: number): TickReport {
  const t0 = performance.now();
  observe_render_cost();
  const phase_ms: Record<Phase, number> = { interaction: 0, visible: 0, content: 0, cosmetic: 0, report: 0 };
  // Sized properly once the interaction phase has said whether the view moved.
  const ctx: TickContext = {
    state,
    delta,
    budget: new FrameBudget(0, t0),
    phase_ms,
    view_moving: () => (moved_at = t0),
    frame_ms: 0,
    settled: false,
  };

  // What changed since the last frame reaches its listeners before anything
  // reads the world this frame — and outside the write that changed it.
  world_wakers.pump();
  run_phase("interaction", ctx);
  ctx.settled = t0 - moved_at >= SETTLE_MS;
  if (!ctx.settled && !prev_settled) observe_frame_gap(delta);
  prev_settled = ctx.settled;
  ctx.frame_ms = frame_ms();
  ctx.budget = new FrameBudget(derive_budget_ms(ctx.frame_ms, ctx.settled), t0);
  ctx.budget.reserve_ms = ctx.budget.total_ms * CONTENT_SHARE;
  run_phase("visible", ctx);
  ctx.budget.reserve_ms = 0;
  for (const phase of PHASES) if (phase !== "interaction" && phase !== "visible") run_phase(phase, ctx);

  last_report = { budget_ms: ctx.budget.total_ms, frame_ms: ctx.frame_ms, settled: ctx.settled, phase_ms, total_ms: performance.now() - t0 };
  return last_report;
}

/**
 * The one frame loop in the world: an animation frame runs the tick, and
 * only when something asked for one — a gesture, a damping step still
 * settling, a build with more to commit, the module's own view_dirty. A
 * frame nobody asked for is skipped, so an idle view costs nothing. Mount
 * once, before anything that registers a step, over `element`, whose size
 * is the viewport's.
 */
export function TickDriver({ element }: { element: HTMLElement | null }) {
  useLayoutEffect(() => {
    if (!element) return;
    const state: TickState = {
      invalidate,
      size: { width: Math.max(1, element.clientWidth), height: Math.max(1, element.clientHeight) },
      viewport: { dpr: Math.min(post_cfg.DPR[1], Math.max(post_cfg.DPR[0], globalThis.devicePixelRatio || 1)) },
      clock: { getElapsedTime: () => (performance.now() - clock_start) / 1000 },
    };
    const clock_start = performance.now();
    let pending = 0;
    let last = performance.now();
    let alive = true;
    function invalidate(): void {
      if (pending || !alive) return;
      pending = requestAnimationFrame(frame);
    }
    function frame(now: number): void {
      pending = 0;
      if (!alive) return;
      // A step's ask during the tick lands after it: the frame it asks for
      // is the next one.
      const delta = Math.min(0.1, Math.max(0, (now - last) / 1000));
      last = now;
      run_tick(state, delta);
    }
    const observer = new ResizeObserver(() => {
      state.size.width = Math.max(1, element.clientWidth);
      state.size.height = Math.max(1, element.clientHeight);
      state.viewport.dpr = Math.min(post_cfg.DPR[1], Math.max(post_cfg.DPR[0], globalThis.devicePixelRatio || 1));
      invalidate();
    });
    observer.observe(element);
    const unbind = bind_tick_request(invalidate);
    invalidate();
    return () => {
      alive = false;
      if (pending) cancelAnimationFrame(pending);
      observer.disconnect();
      unbind();
    };
  }, [element]);
  return null;
}
