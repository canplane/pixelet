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

import type { u32 } from "@xpute/core/kit/word.ts";
import { Vector } from "@xpute/core/ds/arena.ts";
import { fnv1a64_str } from "@xpute/core/kit/hash.ts";
import { I64_KEYS, IndexedMap } from "@xpute/core/ds/map.ts";

import { CORE_ALLOC } from "@/render/layer/pipeline/core_pool.ts";

import type { Demand, Phases } from "./demand.ts";
import { ledger_record, type Outcome } from "./ledger.ts";

// Browsers cap ~6 concurrent connections per origin anyway; matching that
// keeps the queue — not the socket pool — as the thing actually ordering
// work. A knob, not a computed value. Exported for the quota test.
export const MAX_INFLIGHT = 6;

/**
 * Every demand the scheduler holds, queued or in flight, as one table: the
 * key's 64-bit hash in an IndexedMap in the core memory, the state,
 * category, priority and clock lanes at its position, and the two things
 * that are objects by nature — the demand (a closure) and its
 * AbortController — as handles there. Queued and running were two Maps of
 * entry objects; they are one state lane now, and a pump is a walk over it.
 *
 * A key is a string by contract (`<category>:<page>`, or a search query),
 * so it is hashed to the table's key and the stored demand's own key is
 * compared on every hit — a 64-bit collision among tens of keys is not
 * expected, and is loud rather than silent if it ever happens.
 */
const QUEUED = 1;
const RUNNING = 2;

const LANE = { init_cap: 64, alloc: CORE_ALLOC } as const;
const index = new IndexedMap(I64_KEYS, 64, CORE_ALLOC);
const state = new Vector(Uint8Array, LANE);
const category_id = new Vector(Uint8Array, LANE);
const priority = new Vector(Float64Array, LANE);
const enqueue_ms = new Vector(Float64Array, LANE);
const start_ms = new Vector(Float64Array, LANE);
const demands: Demand[] = [];
const controllers: (AbortController | null)[] = [];

/** Categories are a handful of names; a lane holds them as small ids. */
const category_names: string[] = [];
function category_of(name: string): u32 {
  let id = category_names.indexOf(name);
  if (id < 0) id = category_names.push(name) - 1;
  return id;
}

const key_hash = fnv1a64_str;

function find(key: string): number {
  const pos = index.find(key_hash(key));
  if (pos >= 0 && demands[pos].key !== key) {
    console.warn(`[sched/fetch] key hash collision: "${key}" vs "${demands[pos].key}"`);
    return -1;
  }
  return pos;
}

function add(demand: Demand): u32 {
  const pos = index.insert(key_hash(demand.key));
  state.set(pos, QUEUED);
  category_id.set(pos, category_of(demand.category));
  priority.set(pos, demand.priority);
  enqueue_ms.set(pos, performance.now());
  start_ms.set(pos, -1);
  demands[pos] = demand;
  controllers[pos] = null;
  return pos;
}

function remove(pos: u32): void {
  const from = index.remove_at(pos);
  if (from !== pos) {
    state.mem[pos] = state.mem[from];
    category_id.mem[pos] = category_id.mem[from];
    priority.mem[pos] = priority.mem[from];
    enqueue_ms.mem[pos] = enqueue_ms.mem[from];
    start_ms.mem[pos] = start_ms.mem[from];
    demands[pos] = demands[from];
    controllers[pos] = controllers[from];
  }
  state.truncate(from);
  category_id.truncate(from);
  priority.truncate(from);
  enqueue_ms.truncate(from);
  start_ms.truncate(from);
  demands.length = from;
  controllers.length = from;
}

function count(which: u32): u32 {
  let n = 0;
  for (let pos = 0; pos < index.size; pos++) if (state.mem[pos] === which) n++;
  return n;
}

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
  const id = category_names.indexOf(category);
  if (id < 0) return 0;
  let n = 0;
  for (let pos = 0; pos < index.size; pos++) if (category_id.mem[pos] === id) n++;
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

function settle(key: string, demand: Demand, enqueued: number, started: number, outcome: Outcome, phases?: Phases, error?: string): void {
  // The slot may already be gone: a reset aborts and forgets in one step.
  const pos = find(key);
  if (pos >= 0 && demands[pos] === demand) remove(pos);
  const end_ms = performance.now();
  ledger_record({
    key,
    category: demand.category,
    priority: demand.priority,
    enqueue_ms: enqueued,
    start_ms: started,
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

const s_order = new Vector(Uint32Array, LANE);

function pump(): void {
  let inflight = count(RUNNING);
  if (inflight >= MAX_INFLIGHT || inflight === index.size) return;

  // Small N (tens of pages at most) — a sort per pump is cheaper to read
  // than maintaining a heap, and pump runs once per tick, not per frame.
  s_order.truncate(0);
  for (let pos = 0; pos < index.size; pos++) if (state.mem[pos] === QUEUED) s_order.push(pos);
  const by_priority = s_order.mem.subarray(0, s_order.len);
  by_priority.sort((a, b) => priority.mem[a] - priority.mem[b]);

  for (const pos of by_priority) {
    if (inflight >= MAX_INFLIGHT) break;
    inflight++;

    const demand = demands[pos];
    const key = demand.key;
    const controller = new AbortController();
    state.mem[pos] = RUNNING;
    controllers[pos] = controller;
    const started = performance.now();
    start_ms.mem[pos] = started;
    const enqueued = enqueue_ms.mem[pos];

    demand
      .run(controller.signal)
      .then((phases) => settle(key, demand, enqueued, started, "ok", phases || undefined))
      .catch((err) => {
        const aborted = controller.signal.aborted;
        if (!aborted) console.error(`[sched/${key}] failed:`, err);
        settle(key, demand, enqueued, started, aborted ? "aborted" : "error", undefined, aborted ? undefined : error_reason(err));
      });
  }
}

/**
 * Replaces one category's wishlist. Demands for other categories are
 * untouched. New keys are queued, keys already queued pick up the new
 * priority, keys in flight are left running; anything in the category that
 * isn't in `demands` is dropped or aborted.
 */
export function fetch_sync(category: string, wanted: readonly Demand[]): void {
  const id = category_of(category);
  const keep = new Set(wanted.map((d) => d.key));

  // yield: shed this category's demands that are no longer wanted. Backwards,
  // since a removal moves the last slot into the hole.
  for (let pos = index.size - 1; pos >= 0; pos--) {
    if (category_id.mem[pos] !== id || keep.has(demands[pos].key)) continue;
    if (state.mem[pos] === QUEUED) remove(pos);
    else controllers[pos]?.abort();
  }

  // admit the new set.
  for (const demand of wanted) {
    const pos = find(demand.key);
    if (pos < 0) {
      add(demand);
      continue;
    }
    if (state.mem[pos] === QUEUED) {
      demands[pos] = demand;
      priority.mem[pos] = demand.priority;
    }
  }

  schedule_pump();
  emit();
}

/** Whether a demand with this key is in flight right now. Read-only, for a
 * caller deciding whether to keep asking for something it has already asked
 * for (see terrain/use_tile_fetcher.ts's in-flight hysteresis). */
export function fetch_running(key: string): boolean {
  const pos = find(key);
  return pos >= 0 && state.mem[pos] === RUNNING;
}

/** Test-only: abort everything in flight and forget the queue, so each test
 * starts against an empty scheduler. Not part of the runtime surface — the
 * app only ever grows or shrinks a wishlist through fetch_sync. */
export function fetch_scheduler_reset(): void {
  for (let pos = 0; pos < index.size; pos++) controllers[pos]?.abort();
  index.clear();
  state.truncate(0);
  category_id.truncate(0);
  priority.truncate(0);
  enqueue_ms.truncate(0);
  start_ms.truncate(0);
  demands.length = 0;
  controllers.length = 0;
}
