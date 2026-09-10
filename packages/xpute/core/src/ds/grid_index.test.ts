// @xpute/core/ds/grid_index.test.ts

import { assert, assertEquals } from "@std/assert";

import { GridIndex, NO_ROOM, PLACED } from "./grid_index.ts";

type Rect = readonly [number, number, number, number];

/** Mapbox's reference viewport and cell, so the numbers here can be checked
 * against the source this follows: 600x600 at 30 px is 400 cells. */
function grid(capacity = 512, cell = 30) {
  return new GridIndex(600, 600, capacity, cell);
}

function overlaps(a: Rect, b: Rect): boolean {
  return a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1];
}

/** Brute force, as the oracle: greedy first-fit over the same sequence. */
function greedy(rects: readonly Rect[]): boolean[] {
  const kept: Rect[] = [];
  return rects.map((r) => {
    if (kept.some((k) => overlaps(r, k))) return false;
    kept.push(r);
    return true;
  });
}

/** Deterministic, so a failure is reproducible without recording a seed. */
function scatter(n: number, size: number, span = 600): Rect[] {
  const out: Rect[] = [];
  let s = 0x6d2b79f5;
  const next = (max: number) => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return ((s >>> 0) / 0x100000000) * max;
  };
  for (let i = 0; i < n; i++) {
    const x = next(span), y = next(span);
    const w = 4 + next(size), h = 4 + next(size / 3);
    out.push([x, y, x + w, y + h]);
  }
  return out;
}

/**
 * The whole contract, and the only thing a caller can rely on: the answer is
 * exactly what a naive first-fit would have produced. The grid is an
 * acceleration structure, so any divergence from brute force is a bug in the
 * acceleration rather than a different policy.
 */
Deno.test("grid_index - placement matches brute-force greedy exactly", () => {
  for (const size of [10, 40, 120]) {
    const rects = scatter(400, size);
    const g = grid();
    g.begin(600, 600);

    const got = rects.map((r) => g.place(...r) === PLACED);
    assertEquals(got, greedy(rects), `size ${size}`);
  }
});

/**
 * The failure a cell grid invites: two rectangles that overlap but share no
 * cell corner, or that share a cell without overlapping. Both are written
 * out rather than scattered, because a random test hits them rarely and a
 * hand-built one hits them every run.
 */
Deno.test("grid_index - overlap is decided by the rectangles, not by their cells", () => {
  const g = grid(512, 30);
  g.begin(600, 600);

  // Same cell, no overlap: both must be placed.
  assertEquals(g.place(1, 1, 5, 5), PLACED);
  assertEquals(g.place(10, 10, 14, 14), PLACED);

  // Different cells, real overlap: a rectangle spanning the boundary must
  // be caught by the neighbour it reaches into.
  assertEquals(g.place(100, 100, 110, 110), PLACED);
  assert(g.place(105, 105, 200, 200) >= 0);

  // Touching edges count as overlap, which is the conservative choice for
  // labels and has to be consistent with the oracle.
  assertEquals(g.place(300, 300, 310, 310), PLACED);
  assert(g.place(310, 310, 320, 320) >= 0);
});

/**
 * A rectangle wider than the grid's own cell must register in every cell it
 * covers, or a later candidate landing in the middle of it would find the
 * space free.
 */
Deno.test("grid_index - a rectangle spanning many cells blocks all of them", () => {
  const g = grid(512, 30);
  g.begin(600, 600);

  assertEquals(g.place(0, 0, 300, 300), PLACED);
  for (const [x, y] of [[10, 10], [150, 150], [290, 290], [0, 299]]) {
    assert(g.place(x, y, x + 2, y + 2) >= 0, `${x},${y} should be blocked`);
  }
  assertEquals(g.place(320, 320, 322, 322), PLACED);
});

/**
 * Order is the priority rule, so it has to actually decide. The same set in
 * the opposite order must keep the other member of a conflicting pair.
 */
Deno.test("grid_index - the caller's order decides which of two conflicting rectangles survives", () => {
  const a: Rect = [50, 50, 90, 70];
  const b: Rect = [60, 55, 100, 75];

  const first = grid();
  first.begin(600, 600);
  assertEquals(first.place(...a), PLACED);
  assert(first.place(...b) >= 0);

  const second = grid();
  second.begin(600, 600);
  assertEquals(second.place(...b), PLACED);
  assert(second.place(...a) >= 0);
});

/**
 * The stamp is what removes the clearing pass, so a stale cell leaking into
 * a later generation is what it risks. A thousand generations of a shifting
 * layout is where that shows.
 */
Deno.test("grid_index - a thousand generations leave no trace of the ones before", () => {
  const g = grid();
  const rects = scatter(200, 30);

  for (let step = 0; step < 1000; step++) {
    const shifted = rects.map((r) => [r[0] + (step % 7), r[1] - (step % 5), r[2] + (step % 7), r[3] - (step % 5)] as Rect);
    g.begin(600, 600);
    const got = shifted.map((r) => g.place(...r) === PLACED);
    assertEquals(got, greedy(shifted), `step ${step}`);
  }
});

/** A resize must reuse the arrays it was constructed with. */
Deno.test("grid_index - a smaller viewport uses fewer cells without reallocating", () => {
  const g = grid();
  const owned = () => Object.values(g).filter((v) => ArrayBuffer.isView(v));

  g.begin(600, 600);
  const before = owned();
  assertEquals(g.place(10, 10, 20, 20), PLACED);

  g.begin(320, 240);
  assertEquals(g.place(10, 10, 20, 20), PLACED);
  assert(g.place(15, 15, 25, 25) >= 0);

  const after = owned();
  assertEquals(after.length, before.length);
  for (let i = 0; i < before.length; i++) assert(after[i] === before[i], `array ${i} was replaced`);
});

/**
 * Capacity is the design here too. A full grid has to refuse rather than
 * grow, and say so, because a caller that silently under-reports occupancy
 * would draw labels on top of each other.
 */
Deno.test("grid_index - capacity and the node pool are refused, not grown", () => {
  const g = new GridIndex(600, 600, 4, 30, 2);
  g.begin(600, 600);

  // Four slots, two nodes each: a one-cell rectangle uses one node, so the
  // slots run out first.
  for (let i = 0; i < 4; i++) assertEquals(g.place(i * 60, 0, i * 60 + 5, 5), PLACED, `slot ${i}`);
  assertEquals(g.place(400, 0, 405, 5), NO_ROOM);
  assertEquals(g.stats().placed, 4);
  assertEquals(g.stats().dropped, 1);

  // A rectangle needing more nodes than the pool has left is refused even
  // with a slot free.
  const h = new GridIndex(600, 600, 8, 30, 1);
  h.begin(600, 600);
  assertEquals(h.place(0, 0, 5, 5), PLACED);
  assertEquals(h.place(200, 200, 400, 400), NO_ROOM);
  assert(h.stats().dropped >= 1);
});

/**
 * Off-screen is not this structure's decision. It knows about pixels, and a
 * rectangle outside them collides with nothing — culling belongs to whatever
 * runs before.
 */
Deno.test("grid_index - a rectangle entirely off-screen collides with nothing", () => {
  const g = grid();
  g.begin(600, 600);

  assertEquals(g.place(10, 10, 50, 50), PLACED);
  assert(g.hit_test(10, 10, 50, 50));
  assertEquals(g.hit_test(-200, -200, -100, -100), false);
  assertEquals(g.hit_test(900, 900, 1000, 1000), false);
});

/**
 * The density case, as with the spatial hash: far more candidates than room.
 * The point is not how many survive but that the answer stays exact when
 * almost everything collides.
 */
Deno.test("grid_index - ten thousand candidates for a screen that fits far fewer", () => {
  const rects = scatter(10000, 20);
  const g = new GridIndex(600, 600, 10000, 30, 8);
  g.begin(600, 600);

  const got = rects.map((r) => g.place(...r) === PLACED);
  assertEquals(got, greedy(rects));

  const kept = got.filter(Boolean).length;
  assert(kept > 0 && kept < 10000, `expected a real cut, kept ${kept}`);
  assertEquals(g.stats().dropped, 0);
});

/**
 * The attribution absorption rests on: a rejected rectangle has to name the
 * one that took its space, and that name has to be the caller's own ordinal,
 * or a caller cannot turn "refused" into "counted somewhere".
 */
Deno.test("grid_index - a blocked rectangle reports which placement blocked it", () => {
  const g = grid(512, 30);
  g.begin(600, 600);

  assertEquals(g.place(0, 0, 20, 20), PLACED); // id 0
  assertEquals(g.place(100, 100, 120, 120), PLACED); // id 1
  assertEquals(g.place(200, 200, 220, 220), PLACED); // id 2

  assertEquals(g.place(10, 10, 15, 15), 0);
  assertEquals(g.place(110, 110, 115, 115), 1);
  assertEquals(g.place(205, 205, 215, 215), 2);

  // A rejection places nothing, so the next free rectangle is still id 3.
  assertEquals(g.place(400, 400, 420, 420), PLACED);
  assertEquals(g.place(405, 405, 410, 410), 3);
});
