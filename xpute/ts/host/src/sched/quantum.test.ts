// @xpute/host/sched/quantum.test.ts

import { assert, assertAlmostEquals, assertEquals } from "@std/assert";

import { FRAME_WINDOW, type Grant, NO_WAKE, Quantum, type QuantumPolicy } from "./quantum.ts";

const POLICY: QuantumPolicy = { margin_share: 0.15, batch_frames: 2, settle_ms: 150 };

/** A window of animation frames `gap_ms` apart, then the grant after a turn that asked for the next edge. */
function gapped(q: Quantum, gap_ms: number): Grant {
  for (let i = 0; i < FRAME_WINDOW; i++) q.gap(gap_ms);
  q.observe(0, q.grant(0), 0);
  return q.grant(0);
}

Deno.test("quantum - the refresh is the display's, read off its animation frames", () => {
  const q = new Quantum(POLICY);
  assertAlmostEquals(gapped(q, 1000 / 165).refresh_ms, 1000 / 165, 1e-9);
  assertAlmostEquals(gapped(q, 1000 / 60).refresh_ms, 1000 / 60, 1e-9);
});

Deno.test("quantum - turns are paced at the most whole refreshes within 60 Hz, one at least", () => {
  const paced = (hz: number) => gapped(new Quantum(POLICY), 1000 / hz).paced_ms;
  assertAlmostEquals(paced(120), 2000 / 120, 1e-9);
  assertAlmostEquals(paced(60), 1000 / 60, 1e-9);
  assertAlmostEquals(paced(144), 2000 / 144, 1e-9);
  assertAlmostEquals(paced(165), 2000 / 165, 1e-9);
  assertAlmostEquals(paced(90), 1000 / 90, 1e-9);
  // A browser capping its frames at 30 Hz (a battery saver): the refresh itself.
  assertAlmostEquals(paced(30), 1000 / 30, 1e-9);
});

Deno.test("quantum - a refresh read a hair long still paces at two on a 120 Hz display", () => {
  const q = new Quantum(POLICY);
  assertAlmostEquals(gapped(q, 8.34).paced_ms, 2 * 8.34, 1e-9);
});

Deno.test("quantum - the pace is the display's: no load moves it", () => {
  const q = new Quantum(POLICY);
  const before = gapped(q, 1000 / 120);
  for (let i = 0; i < 4 * FRAME_WINDOW; i++) q.observe(40, q.grant(0), 0);
  assertEquals(q.grant(0).paced_ms, before.paced_ms);
});

Deno.test("quantum - a gap that is not a frame moves nothing", () => {
  const q = new Quantum(POLICY);
  const before = gapped(q, 1000 / 165);
  q.gap(3000);
  assertEquals(q.grant(0).refresh_ms, before.refresh_ms);
});

Deno.test("quantum - the quota is the paced interval less the margin", () => {
  const q = new Quantum(POLICY);
  const g = gapped(q, 1000 / 120);
  assert(g.interactive);
  assertAlmostEquals(g.quota_ms, g.paced_ms * (1 - POLICY.margin_share), 1e-9);
});

Deno.test("quantum - what a turn overran comes off its own next quota, down to nothing", () => {
  const q = new Quantum(POLICY);
  const full = gapped(q, 1000 / 30);
  q.observe(full.quota_ms + 3, full, 0);
  assertAlmostEquals(q.grant(0).quota_ms, full.quota_ms - 3, 1e-9);
  q.observe(100, full, 0);
  assertEquals(q.grant(0).quota_ms, 0);
});

Deno.test("quantum - input, or a guest asking for the next edge, makes a turn interactive", () => {
  const q = new Quantum(POLICY);
  for (let i = 0; i < FRAME_WINDOW; i++) q.gap(1000 / 30);
  q.observe(0, q.grant(0), NO_WAKE);
  const idle = q.grant(0);
  assert(!idle.interactive);
  assertAlmostEquals(idle.quota_ms, idle.paced_ms * POLICY.batch_frames * (1 - POLICY.margin_share), 1e-9);
  q.input(1000);
  assert(q.grant(1000 + POLICY.settle_ms - 1).interactive);
  assert(!q.grant(1000 + POLICY.settle_ms).interactive);
  q.observe(0, q.grant(0), 0);
  const asked = q.grant(0);
  assert(asked.interactive);
  assertAlmostEquals(asked.quota_ms, asked.paced_ms * (1 - POLICY.margin_share), 1e-9);
});
