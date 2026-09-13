// @xpute/core/ds/spatial_hash.test.ts

import { assert, assertEquals } from "@std/assert";

import { SpatialHash } from "./spatial_hash.ts";
import { lattice_hash3, lattice_hash4 } from "./lattice_hash.ts";

type Point = { id: number; x: number; y: number; z: number };

/** Cell size 1, so a cell coordinate is the floor of a world one and every
 * boundary case can be written down by hand. 1021 is prime and far above
 * the point counts here, which is the sizing Teschner measured. */
function index(cell_size = 1, capacity = 4096) {
  return new SpatialHash(capacity, 1021, cell_size);
}

function fill(h: SpatialHash, points: readonly Point[]): void {
  h.begin();
  for (const p of points) h.insert(p.id, p.x, p.y, p.z);
  h.build();
}

/** Ids the index reports for a query, sorted. Order is not promised —
 * grouping is by bucket, which is a hash — so a test that depended on it
 * would be testing something this does not say. */
function found(h: SpatialHash, x: number, y: number, z: number, radius: number): number[] {
  const out = new Uint32Array(4096);
  const n = h.query(x, y, z, radius, out);
  return [...out.subarray(0, n)].sort((a, b) => a - b);
}

/** Brute force, as the oracle. */
function within(points: readonly Point[], x: number, y: number, z: number, radius: number): number[] {
  return points
    .filter((p) => Math.abs(p.x - x) <= radius && Math.abs(p.y - y) <= radius && Math.abs(p.z - z) <= radius)
    .map((p) => p.id)
    .sort((a, b) => a - b);
}

/** Deterministic, so a failure is reproducible without a seed to record. */
function scatter(n: number, span: number): Point[] {
  const out: Point[] = [];
  let s = 0x2545f491;
  const next = () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return ((s >>> 0) / 0x100000000) * span - span / 2;
  };
  for (let i = 0; i < n; i++) out.push({ id: i, x: next(), y: next(), z: next() });
  return out;
}

/**
 * The only invariant that matters for a broadphase. False positives are
 * permitted by the contract and cost the caller one exact test; a false
 * negative is an object that silently does not exist, which is the failure
 * this whole structure has to not have.
 */
Deno.test("spatial_hash - a query never misses a point inside its box", () => {
  const points = scatter(2000, 200);
  const h = index();
  fill(h, points);

  for (let t = 0; t < 200; t++) {
    const probe = points[(t * 97) % points.length];
    for (const radius of [0.5, 1, 3.5]) {
      const expected = within(points, probe.x, probe.y, probe.z, radius);
      const reported = new Set(found(h, probe.x, probe.y, probe.z, radius));
      for (const id of expected) assert(reported.has(id), `missed ${id} at r=${radius}`);
    }
  }
});

/**
 * The bound on the other side. Nothing outside the cells the box touches may
 * come back, which is what makes "false positives out to one cell" a
 * statement rather than a hope — and it is also the assertion that catches a
 * hash collision being treated as a hit.
 */
Deno.test("spatial_hash - every reported point lies within one cell of the box", () => {
  const points = scatter(2000, 200);
  const by_id = new Map(points.map((p) => [p.id, p]));
  const cell = 2;
  const h = index(cell);
  fill(h, points);

  for (let t = 0; t < 200; t++) {
    const probe = points[(t * 53) % points.length];
    const radius = 3;
    for (const id of found(h, probe.x, probe.y, probe.z, radius)) {
      const p = by_id.get(id)!;
      // One cell of quantisation on each side, and nothing beyond it: this
      // is the assertion a bucket-level filter cannot pass, since a hash
      // collision would put an arbitrary point in the answer.
      const slack = radius + cell;
      assert(
        Math.abs(p.x - probe.x) <= slack && Math.abs(p.y - probe.y) <= slack && Math.abs(p.z - probe.z) <= slack,
        `${id} is further than one cell outside the box`,
      );
    }
  }
});

/**
 * A point exactly on a cell boundary belongs to one cell, and a query
 * touching that boundary has to visit both sides. Written out by hand
 * because a scatter almost never lands on an integer.
 */
Deno.test("spatial_hash - points on a cell boundary are found from either side", () => {
  const points: Point[] = [
    { id: 1, x: 0, y: 0, z: 0 },
    { id: 2, x: -0.000001, y: 0, z: 0 },
    { id: 3, x: 0.999999, y: 0, z: 0 },
    { id: 4, x: 1, y: 0, z: 0 },
  ];
  const h = index();
  fill(h, points);

  assertEquals(found(h, 0, 0, 0, 0.000002).includes(2), true);
  assertEquals(found(h, 0.9999995, 0, 0, 0.0000015).includes(4), true);
  for (const p of points) assert(found(h, p.x, p.y, p.z, 0).includes(p.id), `${p.id} missing from its own cell`);
});

/**
 * The generation stamp is what removes the clearing pass, so a stale bucket
 * leaking into a later generation is the failure mode it introduces. A
 * thousand rebuilds of a moving population is where that would show.
 */
Deno.test("spatial_hash - a thousand rebuilds leave no trace of the ones before", () => {
  const h = index();
  const points = scatter(500, 100);

  let last: number[] = [];
  for (let step = 0; step < 1000; step++) {
    // Move everything, so a leaked bucket would report a position nothing
    // occupies any more rather than one that happens to still be right.
    const moved = points.map((p) => ({ ...p, x: p.x + step * 0.37, y: p.y - step * 0.11, z: p.z }));
    fill(h, moved);
    last = found(h, moved[0].x, moved[0].y, moved[0].z, 2);
    for (const id of within(moved, moved[0].x, moved[0].y, moved[0].z, 2)) assert(last.includes(id), `step ${step} missed ${id}`);
  }
  assertEquals(h.stats().dropped, 0);
});

/**
 * The whole point of the counting-sort build. If a rebuild allocated, it
 * would be doing so once a frame, which is the cost this structure exists
 * to not have — and the only way to observe that from inside the language
 * is that nothing it owns changes identity.
 */
Deno.test("spatial_hash - a rebuild allocates nothing it does not already own", () => {
  const h = index();
  const points = scatter(1000, 100);

  const owned = () => Object.values(h).filter((v) => ArrayBuffer.isView(v));
  fill(h, points);
  const before = owned();

  for (let step = 0; step < 50; step++) {
    fill(h, points.map((p) => ({ ...p, x: p.x + step })));
  }
  const after = owned();

  assertEquals(after.length, before.length);
  for (let i = 0; i < before.length; i++) assert(after[i] === before[i], `array ${i} was replaced`);
});

/**
 * Capacity is the design, so exceeding it must be visible rather than
 * absorbed: a structure meant to keep the live count constant cannot be the
 * thing that quietly grows.
 */
Deno.test("spatial_hash - capacity is refused, not grown", () => {
  const h = index(1, 10);
  h.begin();
  for (let i = 0; i < 25; i++) assertEquals(h.insert(i, i, 0, 0), i < 10);
  h.build();

  const s = h.stats();
  assertEquals(s.count, 10);
  assertEquals(s.dropped, 15);
  for (let i = 0; i < 10; i++) assert(found(h, i, 0, 0, 0).includes(i));
});

/**
 * Density is the regime this was chosen for — the case an octree degenerates
 * on. Ten thousand points inside a couple of cells must still answer, and
 * the buckets they land in must be the few their positions imply rather than
 * spread by a weak hash.
 */
Deno.test("spatial_hash - ten thousand points in one small volume still resolve", () => {
  const h = index(1, 16384);
  const points: Point[] = [];
  let s = 0x9e3779b9;
  const next = () => {
    s ^= s << 13;
    s ^= s >>> 17;
    s ^= s << 5;
    return ((s >>> 0) / 0x100000000) * 2;
  };
  for (let i = 0; i < 10000; i++) points.push({ id: i, x: next(), y: next(), z: next() });
  fill(h, points);

  assertEquals(h.stats().count, 10000);
  assert(h.stats().touched <= 27, `expected a handful of buckets, got ${h.stats().touched}`);

  const out = new Uint32Array(16384);
  const n = h.query(1, 1, 1, 1, out);
  assertEquals(new Set(out.subarray(0, n)).size, 10000);
});

/**
 * The counterpart to the bound above, stated as the thing that would break
 * it: with far more distinct cells than buckets, collisions are certain, so
 * a query that filtered only by bucket would return points from anywhere.
 * Nothing here does.
 */
Deno.test("spatial_hash - a saturated table returns no id twice and none from a colliding cell", () => {
  // 61 buckets against a scatter over a million cells: every bucket holds
  // several unrelated cells.
  const h = new SpatialHash(4096, 61, 1);
  const points = scatter(1500, 100);
  const by_id = new Map(points.map((p) => [p.id, p]));
  fill(h, points);

  const out = new Uint32Array(4096);
  for (let t = 0; t < 100; t++) {
    const probe = points[(t * 31) % points.length];
    const n = h.query(probe.x, probe.y, probe.z, 1, out);
    const ids = [...out.subarray(0, n)];
    assertEquals(new Set(ids).size, ids.length, "an id was reported twice");
    for (const id of ids) {
      const p = by_id.get(id)!;
      assert(Math.abs(p.x - probe.x) <= 2 && Math.abs(p.y - probe.y) <= 2 && Math.abs(p.z - probe.z) <= 2, `${id} came from a colliding cell`);
    }
    for (const id of within(points, probe.x, probe.y, probe.z, 1)) assert(ids.includes(id), `missed ${id}`);
  }
});

Deno.test("lattice_hash - a tier folded in separates a parent's address from its child's", () => {
  assertEquals(lattice_hash3(3, -4, 5), lattice_hash4(3, -4, 5, 0));
  assert(lattice_hash4(1, 1, 1, 0) !== lattice_hash4(1, 1, 1, 1));
});
