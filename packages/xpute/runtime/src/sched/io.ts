// @xpute/runtime/sched/io.ts

/**
 * A guest's I/O queue: the asynchronous half of its scheduler, beside the
 * tick.
 *
 * The guest does no I/O. It says what it wants and in what order, and the
 * central scheduler — which owns the connections, the way an OS owns its
 * devices — runs what the queue starts, within the credits it grants: the
 * pump starts no more than that many at once. What was started and what was
 * aborted wait here for the outside to take, and the outside settles each.
 *
 * Two levers, no feedback control:
 *
 * - **quota:** at most the granted credits run at once, across every
 *   category, ordered by priority — a near page of one category beats a far
 *   page of another.
 * - **yield:** `sync` takes a category's *entire* current wishlist. Anything
 *   of that category queued or in flight and no longer wished for is dropped
 *   (queued) or aborted (in flight), freeing its slot for what is wanted now.
 *
 * **One pump a turn, after every category has submitted.** With a pump per
 * submission, whichever category synced first won outright: nine demands of
 * one filled every slot before another had submitted anything, and a page at
 * priority 1 sat 358 ms behind pages at priority 2. Priority can only order
 * the queue; it cannot preempt a request already in flight, so submission
 * order was silently beating it. The diff half of `sync` stays immediate on
 * purpose — dropping and aborting must not wait.
 *
 * A demand is a key the guest chose (what it names is the guest's), a
 * category and a priority. The queue holds its capacity in rows and never
 * grows; a demand past them is refused, and its category asks again next
 * turn.
 */

import type { F64Array, U32Array, U64Array, U8Array } from "@xpute/core/abi/array.ts";
import type { f64, i32, u32, u64 } from "@xpute/core/abi/word.ts";
import { now } from "../clock.ts";

export interface Demand {
  /** Stable identity across syncs. Two demands sharing a key are the same
   * work: re-submitting one already queued updates its priority, one
   * already in flight is left alone. */
  key: u64;
  /** Scopes `sync`'s diff. */
  category: u32;
  /** Lower runs sooner. */
  priority: f64;
}

/** A demand that finished, and when it was enqueued, started and ended. */
export interface Settled {
  demand: Demand;
  enqueue_ms: f64;
  start_ms: f64;
  end_ms: f64;
}

export interface IoCounts {
  queued: u32;
  running: u32;
}

const QUEUED = 1;
const RUNNING = 2;

export class IoQueue {
  private readonly key: U64Array;
  private readonly category: U32Array;
  private readonly priority: F64Array;
  private readonly state: U8Array;
  private readonly abort_sent: U8Array;
  private readonly enqueue_ms: F64Array;
  private readonly start_ms: F64Array;
  private len = 0;
  private readonly started_rows: Demand[] = [];
  private readonly aborted_keys: u64[] = [];
  private rev = 0;

  constructor(private readonly capacity: u32) {
    this.key = new BigUint64Array(capacity);
    this.category = new Uint32Array(capacity);
    this.priority = new Float64Array(capacity);
    this.state = new Uint8Array(capacity);
    this.abort_sent = new Uint8Array(capacity);
    this.enqueue_ms = new Float64Array(capacity);
    this.start_ms = new Float64Array(capacity);
  }

  private find(key: u64): i32 {
    for (let k = 0; k < this.len; k++) if (this.key[k] === key) return k;
    return -1;
  }

  private remove(at: u32): void {
    const last = --this.len;
    this.key[at] = this.key[last];
    this.category[at] = this.category[last];
    this.priority[at] = this.priority[last];
    this.state[at] = this.state[last];
    this.abort_sent[at] = this.abort_sent[last];
    this.enqueue_ms[at] = this.enqueue_ms[last];
    this.start_ms[at] = this.start_ms[last];
  }

  private count(state: number): u32 {
    let n = 0;
    for (let k = 0; k < this.len; k++) if (this.state[k] === state) n++;
    return n;
  }

  /** Moves after every queue or in-flight change. */
  revision(): u32 {
    return this.rev;
  }

  counts(): IoCounts {
    return { queued: this.count(QUEUED), running: this.count(RUNNING) };
  }

  /** Queued and in-flight demands of one category. */
  pending_count(category: u32): u32 {
    let n = 0;
    for (let k = 0; k < this.len; k++) if (this.category[k] === category) n++;
    return n;
  }

  /** Whether a demand with this key is in flight. */
  running(key: u64): boolean {
    const at = this.find(key);
    return at >= 0 && this.state[at] === RUNNING;
  }

  /** Whether a demand with this key is queued or in flight. */
  holds(key: u64): boolean {
    return this.find(key) >= 0;
  }

  /** Replaces one category's wishlist; other categories are untouched. New
   * keys are queued, keys already queued take the new priority, keys in
   * flight keep running; anything of the category not in `wanted` is
   * dropped or aborted. Returns how many new demands found no row. */
  sync(category: u32, wanted: readonly Demand[]): u32 {
    // Backwards, since a removal moves the last row into the hole.
    for (let at = this.len - 1; at >= 0; at--) {
      if (this.category[at] !== category || wanted.some((d) => d.key === this.key[at])) continue;
      if (this.state[at] === QUEUED) this.remove(at);
      else if (!this.abort_sent[at] && this.aborted_keys.length < this.capacity) {
        this.aborted_keys.push(this.key[at]);
        this.abort_sent[at] = 1;
      }
    }

    let refused = 0;
    for (const demand of wanted) {
      const at = this.find(demand.key);
      if (at >= 0) {
        if (this.state[at] === QUEUED) {
          this.category[at] = demand.category;
          this.priority[at] = demand.priority;
        }
      } else if (this.len < this.capacity) {
        const k = this.len++;
        this.key[k] = demand.key;
        this.category[k] = demand.category;
        this.priority[k] = demand.priority;
        this.state[k] = QUEUED;
        this.abort_sent[k] = 0;
        this.enqueue_ms[k] = now();
        this.start_ms[k] = -1;
      } else {
        refused++;
      }
    }
    this.rev++;
    return refused;
  }

  /** Starts queued demands, lowest priority first, until `credits` are in
   * flight: the turn's end, once every category has synced. */
  pump(credits: u32): void {
    let inflight = this.count(RUNNING);
    let started = false;
    // Small N: a scan per start is cheaper to read than a heap, and at most
    // `credits` starts happen a turn. Ties go to the earlier row.
    while (inflight < credits && this.started_rows.length < this.capacity) {
      let best = -1;
      for (let k = 0; k < this.len; k++) {
        if (this.state[k] === QUEUED && (best < 0 || this.priority[k] < this.priority[best])) best = k;
      }
      if (best < 0) break;
      this.state[best] = RUNNING;
      this.start_ms[best] = now();
      this.started_rows.push({ key: this.key[best], category: this.category[best], priority: this.priority[best] });
      inflight++;
      started = true;
    }
    if (started) this.rev++;
  }

  /** The demands started since the last clear, for the outside to run. */
  started(): readonly Demand[] {
    return this.started_rows;
  }

  clear_started(): void {
    this.started_rows.length = 0;
  }

  /** The keys aborted since the last clear, for the outside to cancel; each
   * is settled by the outside like any other. */
  aborted(): readonly u64[] {
    return this.aborted_keys;
  }

  clear_aborted(): void {
    this.aborted_keys.length = 0;
  }

  /** The outside's report that a started demand finished, however it did.
   * Null when the key is not in flight — a reset forgets and aborts in one
   * step, so a settle can arrive for a row already gone. */
  settle(key: u64): Settled | null {
    const at = this.find(key);
    if (at < 0 || this.state[at] !== RUNNING) return null;
    const settled = {
      demand: { key: this.key[at], category: this.category[at], priority: this.priority[at] },
      enqueue_ms: this.enqueue_ms[at],
      start_ms: this.start_ms[at],
      end_ms: now(),
    };
    this.remove(at);
    this.rev++;
    return settled;
  }

  /** Aborts everything in flight and forgets the queue. */
  reset(): void {
    for (let at = 0; at < this.len; at++) {
      if (this.state[at] === RUNNING && !this.abort_sent[at] && this.aborted_keys.length < this.capacity) this.aborted_keys.push(this.key[at]);
    }
    this.len = 0;
    this.rev++;
  }
}
