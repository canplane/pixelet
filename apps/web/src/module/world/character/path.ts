// @pxl/web/module/world/character/path.ts

/**
 * Pure companion-movement math for the character pipeline prototype — no
 * renderer import of any kind (no `three`, no React). Positioning/timing
 * logic is kept independent of the current Three.js + R3F renderer on
 * purpose: it is an MVP-stage choice that a future WebGPU + Rust
 * direct-render pipeline could replace, and this module should not need to
 * change if that happens. The Three.js-specific glue (GLTF loading,
 * skinning, animation mixer, writing the result into an Object3D transform)
 * lives in test_walker.tsx.
 */

import type { f64 } from "@xpute/core/kit/word.ts";

export type Vec2 = { x: f64; y: f64 };

/** Bearing from `from` to `to`, in radians (0 = +x). */
export function heading_to(from: Vec2, to: Vec2): f64 {
  return Math.atan2(to.y - from.y, to.x - from.x);
}

export type WalkPlan = {
  start: Vec2;
  target: Vec2;
  /** state.clock.getElapsedTime() at the moment this plan was created. */
  start_time: f64;
  /** Seconds the whole start->target traversal takes, regardless of
   * distance — see sample_walk_plan's own doc comment for why. */
  duration: f64;
};

export type WalkSample = {
  x: f64;
  y: f64;
  /** Direction of travel, or null once arrived — callers should fall back
   * to some other default facing (e.g. facing the camera) in that case. */
  heading: f64 | null;
  arrived: boolean;
};

/**
 * Samples a straight-line move from `plan.start` to `plan.target` that
 * always takes exactly `plan.duration` seconds — distance-independent by
 * design. A companion catching up after a pan should reappear at your side
 * on roughly the same beat whether you panned 1 unit or 1000; a fixed real
 * -world walking *speed* would instead make a big pan take proportionally,
 * sometimes absurdly, longer (this is the same failure mode a fixed speed
 * hit on first spawn — see test_walker.tsx's initial-placement comment —
 * generalized to every subsequent move too).
 */
export function sample_walk_plan(plan: WalkPlan, now: f64): WalkSample {
  const elapsed: f64 = now - plan.start_time;
  const frac: f64 = plan.duration > 0 ? Math.min(1, Math.max(0, elapsed / plan.duration)) : 1;
  const arrived: boolean = frac >= 1;
  return {
    x: plan.start.x + (plan.target.x - plan.start.x) * frac,
    y: plan.start.y + (plan.target.y - plan.start.y) * frac,
    heading: arrived ? null : heading_to(plan.start, plan.target),
    arrived,
  };
}

export type SettleTracker = {
  /** Most recent raw target seen, whether or not it's stopped moving yet. */
  last_seen: Vec2;
  /** When `last_seen` last actually changed. */
  last_change_time: f64;
  /** The last target this tracker has reported as settled. */
  settled: Vec2;
};

export function init_settle_tracker(initial: Vec2, now: f64): SettleTracker {
  // Copied, not aliased: callers (companion.tsx) pass a ref's .current that
  // gets mutated in place every render. Storing that same reference here
  // would make every future "did it change" comparison compare the live
  // object against itself — always equal, so it would never look moved and
  // never settle. See update_settle_tracker's own copy of this note.
  return { last_seen: { x: initial.x, y: initial.y }, last_change_time: now, settled: { x: initial.x, y: initial.y } };
}

/**
 * Tracks a moving target (e.g. map_center) and reports the instant it stops
 * changing, instead of reacting to any one specific gesture. A drag, a
 * click-to-navigate tween, a GPS recenter, a keyboard nudge — anything that
 * moves the target eventually stops changing it, and that's the one moment
 * this fires `just_settled`, regardless of which of those caused it.
 * (camera_status: DRAG/IDLE was tried first — it only fires for drag/
 * rotate/pinch, so a click-driven camera animation that never touches
 * pointer-drag state never triggered a catch-up at all.)
 */
export function update_settle_tracker(
  tracker: SettleTracker,
  target: Vec2,
  now: f64,
  settle_delay: f64,
): { tracker: SettleTracker; just_settled: boolean; moved: boolean } {
  const moved: boolean = target.x !== tracker.last_seen.x || target.y !== tracker.last_seen.y;
  // Copied, not aliased — see init_settle_tracker's doc comment: `target` is
  // typically a mutable ref's .current, live-updated by its owner every
  // render. Storing that reference directly here (instead of a snapshot of
  // its current values) would make it and every later `target` the exact
  // same object, so the `moved` check above would always read false from
  // then on — comparing the live value to itself can never differ.
  const last_seen: Vec2 = moved ? { x: target.x, y: target.y } : tracker.last_seen;
  const last_change_time: f64 = moved ? now : tracker.last_change_time;

  const stable_for: f64 = now - last_change_time;
  const differs_from_settled: boolean = last_seen.x !== tracker.settled.x || last_seen.y !== tracker.settled.y;

  if (stable_for >= settle_delay && differs_from_settled) {
    return { tracker: { last_seen, last_change_time, settled: last_seen }, just_settled: true, moved };
  }
  return { tracker: { last_seen, last_change_time, settled: tracker.settled }, just_settled: false, moved };
}
