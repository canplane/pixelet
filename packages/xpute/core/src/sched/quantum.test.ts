// @xpute/core/sched/quantum.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";

import { FRAME_WINDOW, type Grant, Quantum, type QuantumPolicy } from "./quantum.ts";

const POLICY: QuantumPolicy = { margin_share: 0.15, min_ms: 2, max_ms: 8, batch_ms: 12, outside_ema: 0.1 };

/** A whole window of turns, so what came before is gone. */
function fill(q: Quantum, delta_s: number, outside_ms: number, interactive: boolean): Grant {
  for (let i = 0; i < FRAME_WINDOW; i++) q.observe({ delta_s, turn_ms: outside_ms, used_ms: 0, interactive });
  return q.grant();
}

Deno.test("quantum - the slice follows the display's measured frame, not an assumed 60 Hz", () => {
  const q = new Quantum(POLICY);
  const at_60 = fill(q, 1 / 60, 0, true);
  const at_165 = fill(q, 1 / 165, 0, true);
  assert(at_165.slice_ms < at_60.slice_ms, `165 Hz got ${at_165.slice_ms}, 60 Hz got ${at_60.slice_ms}`);
  assertAlmostEquals(at_165.frame_ms, 1000 / 165, 1e-9);
});

Deno.test("quantum - a gap that is not a frame moves nothing", () => {
  const q = new Quantum(POLICY);
  const before = fill(q, 1 / 165, 0, true);
  q.observe({ delta_s: 3, turn_ms: 0, used_ms: 0, interactive: true });
  assertEquals(q.grant().frame_ms, before.frame_ms);
});

Deno.test("quantum - a still guest's frame gaps do not teach the quantum a faster display", () => {
  const q = new Quantum(POLICY);
  const moving = fill(q, 1 / 165, 0, true);
  // The host catching up after a long still turn: gaps far shorter than any
  // real refresh interval.
  const still = fill(q, 0.002, 0, false);
  assertEquals(still.frame_ms, moving.frame_ms);
});

Deno.test("quantum - what a turn costs outside the slice comes off the next, down to the floor", () => {
  const q = new Quantum(POLICY);
  const light = fill(q, 1 / 60, 10, true).slice_ms;
  const frame = 1000 / 60;
  assertAlmostEquals(light, frame - frame * POLICY.margin_share - 10, 0.01);
  const heavy = fill(q, 1 / 60, 30, true).slice_ms;
  assertEquals(heavy, POLICY.min_ms);
});

Deno.test("quantum - a cheap turn is granted no more than the ceiling, and the batch quantum beside it", () => {
  const q = new Quantum(POLICY);
  const g = fill(q, 1 / 30, 0, true);
  assertEquals(g.slice_ms, POLICY.max_ms);
  assertEquals(g.batch_ms, POLICY.batch_ms);
});
