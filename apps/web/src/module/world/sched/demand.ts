// @pxl/web/module/world/sched/demand.ts

/**
 * One unit of deferrable I/O handed to fetch_scheduler.ts.
 *
 * The scheduler owns ordering, the concurrency quota, and cancellation of
 * demands that stop being wanted; `run` owns the actual transport and
 * whatever it writes on success. The scheduler never reads or writes a
 * store — see fetch_scheduler.ts's own doc for why that line matters.
 *
 * This is the category-B (I/O) shape of the task contract in
 * issue/20260909-cooperative-scheduler-and-task-ledger.md: a black box
 * the scheduler drives by key/priority, nothing more.
 */
/**
 * Optional sub-timings a demand's own `run` can hand back — terrain splitting
 * its network wait from its decode, say. Lands in TaskRecord.phases, so the
 * report can state whether a category is actually network- or memory-bound
 * instead of leaning on category.ts's static guess. Free-form keys: the
 * instrument prints whatever a demand chooses to report.
 */
export type Phases = Record<string, number>;

export type Demand = {
  /**
   * Stable identity across sync() calls, by convention `<category>:<page>`.
   * Two demands sharing a key are the same work: re-submitting one that is
   * already queued updates its priority, one already in flight is left
   * alone.
   */
  key: string;
  /** Coarse grouping — scopes sync()'s diff, and is what map_status and the
   * ledger roll up by. */
  category: string;
  /** Lower runs sooner. Convention: the page's Euclidean distance from the
   * view anchor in whole pages (chunk_range.ts's page_priority), so the
   * nearest page loads first. */
  priority: number;
  /**
   * Performs the fetch and its store write. Receives the scheduler's
   * AbortSignal; when it aborts, `run` must reject (a `fetch(url, { signal })`
   * does this on its own) and must not write anything. May return its own
   * Phases split; returning nothing is fine and means "one opaque step".
   */
  run: (signal: AbortSignal) => Promise<void | Phases>;
};
