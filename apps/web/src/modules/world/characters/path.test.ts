// @pxl/web/modules/world/characters/path.test.ts

import { assertAlmostEquals, assertEquals } from "@std/assert";

import { heading_to, init_settle_tracker, sample_walk_plan, type SettleTracker, update_settle_tracker, type Vec2, type WalkPlan } from "./path.ts";

Deno.test("heading_to - points along +x", () => {
  assertEquals(heading_to({ x: 0, y: 0 }, { x: 5, y: 0 }), 0);
});

Deno.test("heading_to - points along +y", () => {
  assertAlmostEquals(heading_to({ x: 0, y: 0 }, { x: 0, y: 5 }), Math.PI / 2);
});

function plan(start: { x: number; y: number }, target: { x: number; y: number }, duration = 1): WalkPlan {
  return { start, target, start_time: 0, duration };
}

Deno.test("sample_walk_plan - at start_time, sits exactly at start", () => {
  const r = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 10, y: 0 }), 0);
  assertAlmostEquals(r.x, 0);
  assertAlmostEquals(r.y, 0);
  assertEquals(r.arrived, false);
});

Deno.test("sample_walk_plan - halfway through duration is halfway to target, regardless of distance", () => {
  const near = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 2, y: 0 }, 4), 2);
  const far = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 2000, y: 0 }, 4), 2);
  assertAlmostEquals(near.x, 1);
  assertAlmostEquals(far.x, 1000);
  assertEquals(near.arrived, false);
  assertEquals(far.arrived, false);
});

Deno.test("sample_walk_plan - arrives exactly at target once duration has elapsed, for any distance", () => {
  const near = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 2, y: 0 }, 4), 4);
  const far = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 2000, y: 3000 }, 4), 4);
  assertAlmostEquals(near.x, 2);
  assertEquals(near.arrived, true);
  assertEquals(near.heading, null);
  assertAlmostEquals(far.x, 2000);
  assertAlmostEquals(far.y, 3000);
  assertEquals(far.arrived, true);
});

Deno.test("sample_walk_plan - clamps past the end instead of overshooting", () => {
  const r = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 5, y: 0 }, 1), 100);
  assertAlmostEquals(r.x, 5);
  assertEquals(r.arrived, true);
});

Deno.test("sample_walk_plan - heading points from start toward target while still en route", () => {
  const r = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 0, y: 10 }, 4), 1);
  assertAlmostEquals(r.heading!, Math.PI / 2);
});

Deno.test("sample_walk_plan - zero duration arrives immediately", () => {
  const r = sample_walk_plan(plan({ x: 0, y: 0 }, { x: 5, y: 5 }, 0), 0);
  assertEquals(r.arrived, true);
  assertAlmostEquals(r.x, 5);
  assertAlmostEquals(r.y, 5);
});

const SETTLE_DELAY = 0.15;

Deno.test("update_settle_tracker - a target that never moves never settles again", () => {
  const tracker = init_settle_tracker({ x: 1, y: 2 }, 0);
  const r = update_settle_tracker(tracker, { x: 1, y: 2 }, 10, SETTLE_DELAY);
  assertEquals(r.just_settled, false);
});

Deno.test("update_settle_tracker - a continuously moving target (mid-drag) never settles", () => {
  let tracker: SettleTracker = init_settle_tracker({ x: 0, y: 0 }, 0);
  // Simulate a drag updating the target every ~16ms frame, well under the
  // settle delay each step, for a full second — should never fire.
  let t = 0;
  let fired = false;
  for (let i = 0; i < 60; i++) {
    t += 1 / 60;
    const r = update_settle_tracker(tracker, { x: t * 10, y: 0 }, t, SETTLE_DELAY);
    tracker = r.tracker;
    if (r.just_settled) fired = true;
  }
  assertEquals(fired, false);
});

Deno.test("update_settle_tracker - fires once the target stops changing for settle_delay", () => {
  const tracker = init_settle_tracker({ x: 0, y: 0 }, 0);
  // Target jumps to (5, 5) at t=1, then holds steady.
  const during = update_settle_tracker(tracker, { x: 5, y: 5 }, 1, SETTLE_DELAY);
  assertEquals(during.just_settled, false); // just changed, not stable yet

  const too_soon = update_settle_tracker(during.tracker, { x: 5, y: 5 }, 1 + SETTLE_DELAY / 2, SETTLE_DELAY);
  assertEquals(too_soon.just_settled, false);

  // Nudged past the boundary rather than landing exactly on it: (1 +
  // SETTLE_DELAY) - 1 doesn't reliably round-trip to exactly SETTLE_DELAY
  // in floating point, which would make a strict >= comparison flaky here
  // — real callers never hit this exact boundary either.
  const settled = update_settle_tracker(too_soon.tracker, { x: 5, y: 5 }, 1 + SETTLE_DELAY + 0.001, SETTLE_DELAY);
  assertEquals(settled.just_settled, true);
  assertEquals(settled.tracker.settled, { x: 5, y: 5 });
});

Deno.test("update_settle_tracker - only fires once per settle, not on every subsequent stable frame", () => {
  const tracker = init_settle_tracker({ x: 0, y: 0 }, 0);
  const after_change = update_settle_tracker(tracker, { x: 5, y: 5 }, 1, SETTLE_DELAY);
  const first = update_settle_tracker(after_change.tracker, { x: 5, y: 5 }, 1 + SETTLE_DELAY + 0.001, SETTLE_DELAY);
  assertEquals(first.just_settled, true);

  const second = update_settle_tracker(first.tracker, { x: 5, y: 5 }, 1 + SETTLE_DELAY + 1, SETTLE_DELAY);
  assertEquals(second.just_settled, false);
});

Deno.test("update_settle_tracker - a mid-transition jump resets the settle clock (e.g. a tweened camera move)", () => {
  const tracker = init_settle_tracker({ x: 0, y: 0 }, 0);
  const step1 = update_settle_tracker(tracker, { x: 1, y: 0 }, 0.05, SETTLE_DELAY);
  const step2 = update_settle_tracker(step1.tracker, { x: 2, y: 0 }, 0.1, SETTLE_DELAY); // still moving, resets clock
  assertEquals(step2.just_settled, false);
  // Only settle_delay after the LAST change (t=0.1), not the first (t=0.05).
  const not_yet = update_settle_tracker(step2.tracker, { x: 2, y: 0 }, 0.1 + SETTLE_DELAY - 0.01, SETTLE_DELAY);
  assertEquals(not_yet.just_settled, false);
  const now_settled = update_settle_tracker(not_yet.tracker, { x: 2, y: 0 }, 0.1 + SETTLE_DELAY + 0.01, SETTLE_DELAY);
  assertEquals(now_settled.just_settled, true);
});

Deno.test("update_settle_tracker - still detects movement when the caller mutates one shared object in place instead of passing a fresh one each call", () => {
  // This is exactly how companion.tsx calls it: target_ref.current is one
  // object, mutated every render, never replaced. A version of the tracker
  // that stored that reference directly (instead of copying x/y out of it)
  // would end up comparing the live object to itself on every later call —
  // always "equal", so movement would never be detected and it would never
  // settle. This regression-tests that specific failure mode.
  const shared: Vec2 = { x: 0, y: 0 };
  let tracker = init_settle_tracker(shared, 0);

  shared.x = 5;
  shared.y = 5;
  const during = update_settle_tracker(tracker, shared, 1, SETTLE_DELAY);
  tracker = during.tracker;
  assertEquals(during.moved, true);
  assertEquals(during.just_settled, false);

  const settled = update_settle_tracker(tracker, shared, 1 + SETTLE_DELAY + 0.001, SETTLE_DELAY);
  assertEquals(settled.just_settled, true);
  assertEquals(settled.tracker.settled, { x: 5, y: 5 });

  // Mutate the same shared object again — must still be detected as moved,
  // not silently ignored because it's "the same object" as last time.
  shared.x = 9;
  const moved_again = update_settle_tracker(settled.tracker, shared, 1 + SETTLE_DELAY + 0.01, SETTLE_DELAY);
  assertEquals(moved_again.moved, true);
});
