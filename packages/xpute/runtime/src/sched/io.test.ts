// @xpute/runtime/sched/io.test.ts
//
// The outside runs nothing here: a demand starting is its key in `started`
// after the pump, and finishing is `settle`.

import { assert, assertEquals, assertNotEquals } from "@std/assert";

import { type Demand, IoQueue } from "./io.ts";

const CREDITS = 6;

const d = (key: number, category: number, priority: number): Demand => ({ key: BigInt(key), category, priority });

/** The turn's end: the pump, and the keys it started. */
function turn(q: IoQueue): number[] {
  q.pump(CREDITS);
  const keys = q.started().map((x) => Number(x.key));
  q.clear_started();
  return keys;
}

const range = (from: number, to: number) => Array.from({ length: to - from }, (_, i) => from + i);

Deno.test("io - runs at most the credits granted at once, and admits the rest as slots free", () => {
  const q = new IoQueue(32);
  const h = range(0, 9).map((i) => d(i, 0, i));
  q.sync(0, h);
  assertEquals(turn(q), [0, 1, 2, 3, 4, 5]);
  assertEquals(q.pending_count(0), 9);
  q.settle(0n);
  assertEquals(turn(q), [6]);
  for (const x of h) q.settle(x.key);
  assertEquals(turn(q), [7, 8]);
  q.settle(7n);
  q.settle(8n);
  assertEquals(q.pending_count(0), 0);
});

Deno.test("io - the lowest priority number runs first", () => {
  const q = new IoQueue(32);
  q.sync(1, range(100, 106).map((k) => d(k, 1, -1)));
  assertEquals(turn(q).length, 6);
  q.sync(0, [d(1, 0, 10), d(2, 0, 1), d(3, 0, 5)]);
  assertEquals(turn(q), []);
  q.settle(100n);
  assertEquals(turn(q), [2]);
  q.settle(101n);
  assertEquals(turn(q), [3]);
  q.settle(102n);
  assertEquals(turn(q), [1]);
});

Deno.test("io - a later category submitting in the same turn still wins on priority", () => {
  const q = new IoQueue(32);
  q.sync(1, range(10, 16).map((k) => d(k, 1, 5)));
  q.sync(2, [d(20, 2, 0)]);
  const s = turn(q);
  assert(s.includes(20));
  assertEquals(s.filter((k) => k >= 10 && k < 16).length, 5);
});

Deno.test("io - sync aborts an in-flight demand no longer wanted, and says so once", () => {
  const q = new IoQueue(32);
  const keep = d(1, 0, 0);
  const drop = d(2, 0, 1);
  q.sync(0, [keep, drop]);
  assertEquals(turn(q).length, 2);
  q.sync(0, [keep]);
  assertEquals(q.aborted(), [2n]);
  q.clear_aborted();
  q.sync(0, [keep]);
  assertEquals(q.aborted(), []);
  assertNotEquals(q.settle(2n), null);
  assertNotEquals(q.settle(1n), null);
  assertEquals(q.pending_count(0), 0);
});

Deno.test("io - running answers only for keys in flight", () => {
  const q = new IoQueue(32);
  assertEquals(q.running(1n), false);
  q.sync(0, [d(1, 0, 0)]);
  assertEquals(q.running(1n), false); // queued, not yet pumped
  turn(q);
  assertEquals(q.running(1n), true);
  q.settle(1n);
  assertEquals(q.running(1n), false);
});

Deno.test("io - sync drops a queued demand no longer wanted", () => {
  const q = new IoQueue(32);
  const fill = range(100, 106).map((k) => d(k, 1, -1));
  q.sync(1, fill);
  turn(q);
  q.sync(0, [d(50, 0, 0)]);
  assertEquals(turn(q), []);
  assertEquals(q.pending_count(0), 1);
  q.sync(0, []);
  assertEquals(q.pending_count(0), 0);
  for (const x of fill) q.settle(x.key);
  assert(!turn(q).includes(50), "freeing slots must not resurrect it");
});

Deno.test("io - re-submitting a running key does not start a second run", () => {
  const q = new IoQueue(32);
  q.sync(0, [d(1, 0, 0)]);
  assertEquals(turn(q), [1]);
  q.sync(0, [d(1, 0, 0)]);
  assertEquals(turn(q), []);
  assertEquals(q.pending_count(0), 1);
  q.settle(1n);
  assertEquals(q.pending_count(0), 0);
});

Deno.test("io - a full queue refuses what it cannot hold", () => {
  const q = new IoQueue(4);
  assertEquals(q.sync(0, range(0, 6).map((i) => d(i, 0, i))), 2);
  assertEquals(q.pending_count(0), 4);
});

Deno.test("io - fewer credits than are running start nothing more", () => {
  const q = new IoQueue(32);
  q.sync(0, range(0, 7).map((i) => d(i, 0, i)));
  assertEquals(turn(q).length, 6);
  q.pump(3);
  assertEquals(q.started(), []);
});

Deno.test("io - settle hands back the demand, and when it was enqueued, started and ended", () => {
  const q = new IoQueue(32);
  const x = d(1, 3, 2);
  q.sync(3, [x]);
  turn(q);
  const s = q.settle(1n);
  assert(s !== null);
  assertEquals(s.demand, x);
  assert(s.enqueue_ms <= s.start_ms && s.start_ms <= s.end_ms);
  assertEquals(q.settle(1n), null);
});
