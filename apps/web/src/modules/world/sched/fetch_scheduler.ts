// @pxl/web/modules/world/sched/fetch_scheduler.ts

/**
 * Main-thread cooperative fetch scheduler — the category-B (I/O) half of
 * discussions/20260909-cooperative-scheduler-and-task-ledger.md.
 *
 * Two levers, no feedback control:
 *
 * - **quota:** at most MAX_INFLIGHT requests run at once, across every
 *   category. Terrain and POI compete for the same slots, ordered by
 *   priority — a near terrain page beats a far POI page. The pump that
 *   fills those slots is coalesced to one per tick (see schedule_pump)
 *   so every category submitting in the same tick competes fairly.
 * - **yield:** sync() takes a category's *entire* current wishlist. Anything
 *   that was queued or in flight for that category and is no longer wished
 *   for is dropped (queued) or aborted (in flight), freeing its slot for
 *   what's wanted now.
 *
 * Callers (terrain/use_tile_fetcher.ts, poi/use_poi_fetcher.ts) hand over a
 * Demand[] and nothing else. The scheduler never touches a store, so "which
 * pages do I want" and "how do I fetch one" stay where they were; only
 * "when / in what order / how many at once" moved here. Module-level state,
 * not a store: this is scheduling wiring, and a subscriber shouldn't
 * re-render just because the queue shifted — the one reactive readout
 * (map_status) goes through fetch_subscribe.
 */

import type { Demand, Phases } from "./demand.ts";
import { ledger_record, type Outcome } from "./ledger.ts";

// Browsers cap ~6 concurrent connections per origin anyway; matching that
// keeps the queue — not the socket pool — as the thing actually ordering
// work. A knob, not a computed value. Exported for the quota test.
export const MAX_INFLIGHT = 6;

type Entry = {
  demand: Demand;
  enqueue_ms: number;
  start_ms: number | null;
  /** set when the entry moves from queued to running. */
  controller: AbortController | null;
};

const queued = new Map<string, Entry>();
const running = new Map<string, Entry>();

type Listener = () => void;
const listeners = new Set<Listener>();

/** Notified after every queue/running change — the hook map_status uses to
 * follow the terrain queue. Returns an unsubscribe. */
export function fetch_subscribe(fn: Listener): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function emit(): void {
  for (const fn of listeners) fn();
}

/** Queued + in-flight demands in one category — map_status reads this. */
export function fetch_pending_count(category: string): number {
  let n = 0;
  for (const e of queued.values()) if (e.demand.category === category) n++;
  for (const e of running.values()) if (e.demand.category === category) n++;
  return n;
}

/**
 * A short, stable label for why a task failed — what the ledger groups
 * failures by. The repositories throw `"[category/page] http 503"`, so the
 * status is the useful part when there is one; anything else keeps its
 * name and the start of its message.
 */
export function error_reason(err: unknown): string {
  const message = err instanceof Error ? err.message : String(err);
  const http = /\bhttp (\d{3})\b/.exec(message);
  if (http) return `http ${http[1]}`;
  const name = err instanceof Error ? err.name : typeof err;
  return `${name}: ${message}`.slice(0, 80);
}

function settle(key: string, entry: Entry, outcome: Outcome, phases?: Phases, error?: string): void {
  running.delete(key);
  const end_ms = performance.now();
  ledger_record({
    key,
    category: entry.demand.category,
    priority: entry.demand.priority,
    enqueue_ms: entry.enqueue_ms,
    start_ms: entry.start_ms,
    end_ms,
    transport_ms: 0,
    phases,
    outcome,
    error,
  });
  schedule_pump();
  emit();
}

let pump_scheduled = false;

/**
 * Coalesces every pump request in the current tick into one.
 *
 * Measured motivation: with a synchronous pump, whichever subsystem called
 * sync() first won outright — MapScene calls useTileFetcher() before
 * usePoiFetcher(), so terrain's nine demands filled all MAX_INFLIGHT slots
 * before POI had submitted anything, and a POI page at priority 1 sat 358ms
 * behind terrain pages at priority 2. Priority can only order the queue; it
 * cannot preempt a request already in flight, so submission order was
 * silently beating it. Deferring the pump one microtask lets every sync()
 * in the same tick land first, and the single pump that follows picks the
 * top MAX_INFLIGHT across all of them.
 *
 * The diff half of sync() stays synchronous on purpose — dropping and
 * aborting superseded demands must not wait for a microtask.
 */
function schedule_pump(): void {
  if (pump_scheduled) return;
  pump_scheduled = true;
  queueMicrotask(() => {
    pump_scheduled = false;
    pump();
  });
}

function pump(): void {
  if (running.size >= MAX_INFLIGHT || queued.size === 0) return;

  // Small N (tens of pages at most) — a sort per pump is cheaper to read
  // than maintaining a heap, and pump runs once per tick, not per frame.
  const by_priority = [...queued.values()].sort((a, b) => a.demand.priority - b.demand.priority);

  for (const entry of by_priority) {
    if (running.size >= MAX_INFLIGHT) break;

    const key = entry.demand.key;
    queued.delete(key);
    running.set(key, entry);

    const controller = new AbortController();
    entry.controller = controller;
    entry.start_ms = performance.now();

    entry.demand
      .run(controller.signal)
      .then((phases) => settle(key, entry, "ok", phases || undefined))
      .catch((err) => {
        const aborted = controller.signal.aborted;
        if (!aborted) console.error(`[sched/${key}] failed:`, err);
        settle(key, entry, aborted ? "aborted" : "error", undefined, aborted ? undefined : error_reason(err));
      });
  }
}

/**
 * Replaces one category's wishlist. Demands for other categories are
 * untouched. New keys are queued, keys already queued pick up the new
 * priority, keys in flight are left running; anything in the category that
 * isn't in `demands` is dropped or aborted.
 */
export function fetch_sync(category: string, demands: readonly Demand[]): void {
  const wanted = new Set(demands.map((d) => d.key));

  // yield: shed this category's demands that are no longer wanted.
  for (const [key, entry] of queued) {
    if (entry.demand.category === category && !wanted.has(key)) queued.delete(key);
  }
  for (const [key, entry] of running) {
    if (entry.demand.category === category && !wanted.has(key)) entry.controller?.abort();
  }

  // admit the new set.
  for (const demand of demands) {
    if (running.has(demand.key)) continue;
    const q = queued.get(demand.key);
    if (q) {
      q.demand = demand;
      continue;
    }
    queued.set(demand.key, { demand, enqueue_ms: performance.now(), start_ms: null, controller: null });
  }

  schedule_pump();
  emit();
}

/** Whether a demand with this key is in flight right now. Read-only, for a
 * caller deciding whether to keep asking for something it has already asked
 * for (see terrain/use_tile_fetcher.ts's in-flight hysteresis). */
export function fetch_running(key: string): boolean {
  return running.has(key);
}

/** Test-only: abort everything in flight and forget the queue, so each test
 * starts against an empty scheduler. Not part of the runtime surface — the
 * app only ever grows or shrinks a wishlist through fetch_sync. */
export function fetch_scheduler_reset(): void {
  for (const entry of running.values()) entry.controller?.abort();
  running.clear();
  queued.clear();
}
