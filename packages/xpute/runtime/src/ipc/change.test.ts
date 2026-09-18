// @xpute/runtime/ipc/change.test.ts

import { assert, assertEquals, assertThrows } from "@std/assert";

import { ChangeLog, type Cursor, type Topic } from "./change.ts";
import { ChangeWaker } from "./change_waker.ts";

const PAGE: Topic = 0;
const CELL: Topic = 1;

/** A reader's journal loop, the way a caller writes one: interest is a
 * comparison over the span. */
function read(log: ChangeLog, cursor: Cursor, topic: Topic): { keys: bigint[]; revisions: number[]; next: Cursor } | "lost" {
  if (!log.readable(cursor)) return "lost";
  const keys: bigint[] = [], revisions: number[] = [];
  for (let p = cursor; p < log.head; p++) {
    const i = log.slot(p);
    if (log.topic[i] !== topic) continue;
    keys.push(log.key[i]);
    revisions.push(log.revision[i]);
  }
  return { keys, revisions, next: log.head };
}

Deno.test("change - a bump advances the topic's tick and becomes the key's revision", () => {
  const log = new ChangeLog({ topics: 2, capacity: 16 });
  assertEquals(log.revision_of(PAGE, 7n), 0);
  assertEquals(log.bump(PAGE, 7n), 1);
  assertEquals(log.bump(PAGE, 9n), 2);
  assertEquals(log.bump(PAGE, 7n), 3);
  assertEquals(log.revision_of(PAGE, 7n), 3);
  assertEquals(log.revision_of(PAGE, 9n), 2);
  assertEquals(log.tick(PAGE), 3);
  assertEquals(log.tick(CELL), 0, "topics count separately");
});

Deno.test("change - a key forgotten and changed again never repeats a revision a reader may hold", () => {
  const log = new ChangeLog({ topics: 1, capacity: 16 });
  log.bump(PAGE, 1n);
  const seen = log.revision_of(PAGE, 1n);
  log.forget(PAGE, 1n);
  assertEquals(log.revision_of(PAGE, 1n), 0);
  log.bump(PAGE, 1n);
  assert(log.revision_of(PAGE, 1n) !== seen);
});

Deno.test("change - forgetting moves the last key into the hole and keeps every other revision", () => {
  const log = new ChangeLog({ topics: 1, capacity: 16 });
  for (const k of [10n, 20n, 30n, 40n]) log.bump(PAGE, k);
  log.forget(PAGE, 20n);
  assertEquals([10n, 20n, 30n, 40n].map((k) => log.revision_of(PAGE, k)), [1, 0, 3, 4]);
  log.forget(PAGE, 99n);
  assertEquals(log.revision_of(PAGE, 40n), 4, "forgetting an unknown key changes nothing");
});

Deno.test("change - a journal read returns the reader's topic from its cursor, and the head to resume from", () => {
  const log = new ChangeLog({ topics: 2, capacity: 16 });
  const start = log.head;
  log.bump(PAGE, 1n);
  log.bump(CELL, 5n);
  log.bump(PAGE, 2n);
  const first = read(log, start, PAGE);
  assert(first !== "lost");
  assertEquals(first.keys, [1n, 2n]);
  assertEquals(first.revisions, [1, 2]);
  log.bump(PAGE, 1n);
  const second = read(log, first.next, PAGE);
  assert(second !== "lost");
  assertEquals(second.keys, [1n]);
  assertEquals(second.revisions, [3]);
});

Deno.test("change - a cursor the ring has overwritten is lost; one exactly capacity behind is not", () => {
  const log = new ChangeLog({ topics: 1, capacity: 4 });
  const start = log.head;
  for (let k = 0n; k < 4n; k++) log.bump(PAGE, k);
  assert(log.readable(start));
  const whole = read(log, start, PAGE);
  assert(whole !== "lost");
  assertEquals(whole.keys, [0n, 1n, 2n, 3n]);
  log.bump(PAGE, 4n);
  assertEquals(read(log, start, PAGE), "lost");
  assert(log.readable(log.head - 4));
  assertEquals(log.revision_of(PAGE, 0n), 1, "point reads are unaffected by the ring");
});

Deno.test("change - a key past 2^63 is the same key in the table and the journal", () => {
  const log = new ChangeLog({ topics: 1, capacity: 4 });
  const big = (1n << 63n) + 5n;
  log.bump(PAGE, big);
  assertEquals(log.revision_of(PAGE, big), 1);
  assertEquals(BigInt.asUintN(64, log.key[log.slot(0)]), big);
});

Deno.test("change - the topic count is bounded by the u16 a topic travels as", () => {
  assertThrows(() => new ChangeLog({ topics: 0x10001, capacity: 4 }));
  assertThrows(() => new ChangeLog({ topics: 1, capacity: 0 }));
});

Deno.test("waker - a pump wakes the listeners of changed keys once each, and nothing between pumps", () => {
  const log = new ChangeLog({ topics: 2, capacity: 16 });
  const waker = new ChangeWaker(log);
  let a = 0, b = 0, cell = 0;
  waker.subscribe(PAGE, 1n, () => a++);
  const off_b = waker.subscribe(PAGE, 2n, () => b++);
  waker.subscribe(CELL, 1n, () => cell++);

  log.bump(PAGE, 1n);
  log.bump(PAGE, 1n);
  assertEquals([a, b, cell], [0, 0, 0], "the log calls no one");
  assertEquals(waker.pump(), 1);
  assertEquals([a, b, cell], [1, 0, 0], "twice in one span wakes once; the same key in another topic is another key");

  off_b();
  log.bump(PAGE, 2n);
  assertEquals(waker.pump(), 0);
  assertEquals(waker.pump(), 0, "an empty span wakes nothing");
});

Deno.test("waker - a pump whose cursor was overwritten wakes every listener it holds", () => {
  const log = new ChangeLog({ topics: 2, capacity: 2 });
  const waker = new ChangeWaker(log);
  let a = 0, cell = 0;
  waker.subscribe(PAGE, 1n, () => a++);
  waker.subscribe(CELL, 9n, () => cell++);
  for (let k = 10n; k < 15n; k++) log.bump(PAGE, k);
  waker.pump();
  assertEquals([a, cell], [1, 1]);
});

Deno.test("waker - a topic listener hears each changed key of its topic once per pump, and null when the pump was lost", () => {
  const log = new ChangeLog({ topics: 2, capacity: 4 });
  const waker = new ChangeWaker(log);
  const heard: (bigint | null)[] = [];
  const off = waker.subscribe_topic(PAGE, (key) => heard.push(key));
  log.bump(PAGE, 1n);
  log.bump(CELL, 1n);
  log.bump(PAGE, 2n);
  log.bump(PAGE, 1n);
  waker.pump();
  assertEquals(heard, [1n, 2n]);
  heard.length = 0;
  for (let k = 0n; k < 6n; k++) log.bump(PAGE, k);
  waker.pump();
  assertEquals(heard, [null]);
  off();
  log.bump(PAGE, 9n);
  heard.length = 0;
  waker.pump();
  assertEquals(heard, []);
});
