// @xpute/core/sched/quantum.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";

import { FRAME_WINDOW, type Grant, NO_WAKE, Quantum, type QuantumPolicy } from "./quantum.ts";

const POLICY: QuantumPolicy = { margin_share: 0.15, min_ms: 2, max_ms: 8, batch_ms: 12, settle_ms: 150 };

/** A whole window of turns on what each grant said, so what came before is
 * gone; the grant after them. */
function fill(q: Quantum, delta_s: number, turn_ms: number, wake_ms: number): Grant {
  for (let i = 0; i < FRAME_WINDOW; i++) q.observe({ delta_s, turn_ms, grant: q.grant(0), wake_ms });
  return q.grant(0);
}

Deno.test("quantum - the quota follows the display's measured frame, not an assumed 60 Hz", () => {
  const q = new Quantum(POLICY);
  const at_60 = fill(q, 1 / 60, 0, 0);
  const at_165 = fill(q, 1 / 165, 0, 0);
  assert(at_165.quota_ms < at_60.quota_ms, `165 Hz got ${at_165.quota_ms}, 60 Hz got ${at_60.quota_ms}`);
  assertAlmostEquals(at_165.frame_ms, 1000 / 165, 1e-9);
});

Deno.test("quantum - a gap that is not a frame moves nothing", () => {
  const q = new Quantum(POLICY);
  const before = fill(q, 1 / 165, 0, 0);
  q.observe({ delta_s: 3, turn_ms: 0, grant: before, wake_ms: 0 });
  assertEquals(q.grant(0).frame_ms, before.frame_ms);
});

Deno.test("quantum - a still guest's frame gaps do not teach the quantum a faster display", () => {
  const q = new Quantum(POLICY);
  const moving = fill(q, 1 / 165, 0, 0);
  // The host catching up after a long still turn: gaps far shorter than any
  // real refresh interval.
  const still = fill(q, 0.002, 0, NO_WAKE);
  assertEquals(still.frame_ms, moving.frame_ms);
});

Deno.test("quantum - what a turn overran comes off its own next quota, down to the floor", () => {
  const q = new Quantum(POLICY);
  const full = fill(q, 1 / 30, 0, 0);
  assertEquals(full.quota_ms, POLICY.max_ms);
  q.observe({ delta_s: 1 / 30, turn_ms: full.quota_ms + 3, grant: full, wake_ms: 0 });
  assertEquals(q.grant(0).quota_ms, POLICY.max_ms - 3);
  q.observe({ delta_s: 1 / 30, turn_ms: 40, grant: full, wake_ms: 0 });
  assertEquals(q.grant(0).quota_ms, POLICY.min_ms);
});

Deno.test("quantum - input, or a guest asking for the next edge, makes a turn interactive", () => {
  const q = new Quantum(POLICY);
  const idle = fill(q, 1 / 30, 0, NO_WAKE);
  assert(!idle.interactive && idle.quota_ms === POLICY.batch_ms);
  q.input(1000);
  assert(q.grant(1000 + POLICY.settle_ms - 1).interactive);
  assert(!q.grant(1000 + POLICY.settle_ms).interactive);
  const asked = fill(q, 1 / 30, 0, 0);
  assert(asked.interactive && asked.quota_ms === POLICY.max_ms);
});
