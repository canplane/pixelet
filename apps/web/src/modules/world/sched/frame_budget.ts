// @pxl/web/modules/world/sched/frame_budget.ts

/**
 * The category-A (CPU stepped work) kernel of
 * discussions/20260909-cooperative-scheduler-and-task-ledger.md — the piece
 * every per-frame work loop in this app was open-coding.
 *
 * It is deliberately *not* a registry. Deciding what is eligible and in what
 * order is domain work (frustum membership, camera distance, page distance)
 * and stays with each caller; what generalizes is the part underneath: run
 * an ordered list of steps until a wall-time budget or a step cap says stop,
 * without ever starving the queue.
 *
 * **Forward progress is the invariant.** Neither the budget nor the cap
 * applies until at least one step has actually done work. A frame that is
 * already over budget before the loop starts still commits one step, so a
 * single expensive item can never permanently block everything behind it —
 * which is what separates cooperative scheduling from "skip work when
 * busy", and is exactly the rule chunk_stage.tsx's two loops already
 * followed by hand.
 *
 * A step returning `false` means "did not complete work": it doesn't count
 * toward the cap, mirroring the `if (control.commit()) committed++` shape it
 * replaces. It does **not** mean the step was free — see `hard_budget_ms`,
 * which exists because assuming otherwise cost a 4.7-second frame.
 *
 * Shares nothing across calls itself. The frame-wide pool is sched/tick.ts's
 * FrameBudget, which calls this with whatever the frame has left.
 */

export type BudgetOpts = {
  /** wall-time this pass may consume, in ms. Waived while the pass has not
   * yet completed a step — that is the forward-progress guarantee. */
  budget_ms: number;
  /**
   * Absolute ceiling on the pass, never waived.
   *
   * `budget_ms` alone is not enough, and this is the measured reason: a step
   * may cost real time and still report `false`. chunk_stage's `commit()`
   * once scanned for ~15 ms and then deferred, a deliberate hand-off so a
   * cheaper chunk got the frame's slot. With the soft budget waived until
   * something succeeds, a run of expensive deferrals armed nothing: ~2000
   * pending chunks produced a single 4711 ms frame while every individual
   * step stayed under 42 ms.
   *
   * So the soft budget keeps the guarantee (something always runs) and this
   * bounds what the guarantee is allowed to cost. Omit for no ceiling.
   */
  hard_budget_ms?: number;
  /** hard cap on completed steps, independent of time. Omit for no cap. */
  max_steps?: number;
};

export type PassStats = {
  /** steps that reported doing work. */
  ran: number;
  /** items visited (including no-op steps and the one that tripped the stop). */
  visited: number;
  elapsed_ms: number;
  /** true when the loop stopped early — there was more it would have run. */
  stopped_early: boolean;
};

/**
 * Runs `step` over `items` in the order given, stopping once the budget or
 * the step cap is reached — except that the first step to do real work
 * always runs. Returns what it managed.
 */
export function run_under_budget<T>(
  items: readonly T[],
  step: (item: T) => boolean,
  opts: BudgetOpts,
): PassStats {
  const t0 = performance.now();
  const deadline = t0 + opts.budget_ms;
  const hard_deadline = t0 + (opts.hard_budget_ms ?? Infinity);
  const cap = opts.max_steps ?? Infinity;

  let ran = 0;
  let visited = 0;
  let stopped_early = false;

  for (const item of items) {
    const now = performance.now();
    // The ceiling applies even to a pass that has achieved nothing — see
    // hard_budget_ms on why "nothing achieved" is not the same as "nothing
    // spent". Checked first so it can never be out-voted by the waiver below,
    // but still gated on `visited` so the first item is always attempted:
    // forward progress is the invariant, and a caller passing a tiny ceiling
    // should get one bounded attempt rather than a pass that does nothing.
    if (visited > 0 && now >= hard_deadline) {
      stopped_early = true;
      break;
    }
    if (ran > 0 && (ran >= cap || now >= deadline)) {
      stopped_early = true;
      break;
    }
    visited++;
    if (step(item)) ran++;
  }

  return { ran, visited, elapsed_ms: performance.now() - t0, stopped_early };
}
