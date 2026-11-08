// @xpute/runtime/sched/frame_pacing.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import {
  FRAME_WINDOW,
  type FrameGrant,
  FramePacing,
  type FramePacingConfig,
} from "./frame_pacing.ts";
import { NO_WAKE } from "./wake.ts";

const POLICY: FramePacingConfig = {
  pace_hz: 60,
  margin_share: 0.15,
  batch_frames: 2,
  settle_ms: 150,
};

function near(a: number, b: number, tol: number): void {
  assert.ok(Math.abs(a - b) <= tol, `${a} is not within ${tol} of ${b}`);
}

/** Feeds a window of animation frames `gap_ms` apart, then returns the grant
 * after a turn that asked for the next turn at once. */
function grant_after_gaps(q: FramePacing, gap_ms: number): FrameGrant {
  for (let i = 0; i < FRAME_WINDOW; i++) q.gap(gap_ms);
  q.observe(0, q.grant(0), 0);
  return q.grant(0);
}

test("frame pacing - the refresh is the display's, read off its animation frames", () => {
  const q = new FramePacing(POLICY, () => {});
  near(grant_after_gaps(q, 1000 / 165).refresh_ms, 1000 / 165, 1e-9);
  near(grant_after_gaps(q, 1000 / 60).refresh_ms, 1000 / 60, 1e-9);
});

test("frame pacing - before a frame is seen, the refresh is one interval at the program's rate, and the first gaps decide it", () => {
  const q = new FramePacing({ ...POLICY, pace_hz: 30 }, () => {});
  near(q.grant(0).refresh_ms, 1000 / 30, 1e-9);
  near(q.grant(0).paced_ms, 1000 / 30, 1e-9);
  q.gap(1000 / 120);
  near(q.grant(0).refresh_ms, 1000 / 120, 1e-9);
  near(q.grant(0).paced_ms, 4000 / 120, 1e-9);
});

test("frame pacing - turns are paced at the most whole refreshes within the program's rate, one at least", () => {
  const paced = (hz: number) =>
    grant_after_gaps(new FramePacing(POLICY, () => {}), 1000 / hz).paced_ms;
  near(paced(120), 2000 / 120, 1e-9);
  near(paced(60), 1000 / 60, 1e-9);
  near(paced(144), 2000 / 144, 1e-9);
  near(paced(165), 2000 / 165, 1e-9);
  near(paced(90), 1000 / 90, 1e-9);
  // A browser that caps its frames at 30 Hz, as a battery saver does, is
  // paced at the refresh itself.
  near(paced(30), 1000 / 30, 1e-9);
});

test("frame pacing - a refresh read a hair long still paces at two on a 120 Hz display", () => {
  const q = new FramePacing(POLICY, () => {});
  near(grant_after_gaps(q, 8.34).paced_ms, 2 * 8.34, 1e-9);
});

test("frame pacing - no turn's length moves the pace: the load is the program's to judge", () => {
  const q = new FramePacing(POLICY, () => {});
  const before = grant_after_gaps(q, 1000 / 120);
  for (let i = 0; i < 4 * FRAME_WINDOW; i++) q.observe(40, q.grant(0), 0);
  assert.deepStrictEqual(q.grant(0).paced_ms, before.paced_ms);
});

test("frame pacing - turns are paced at the fewest whole refreshes that reach the interval asked", () => {
  const paced = (hz: number, ms: number) => {
    const q = new FramePacing(POLICY, () => {});
    q.pace(ms);
    return grant_after_gaps(q, 1000 / hz).paced_ms;
  };
  near(paced(120, 1000 / 30), 4000 / 120, 1e-9);
  near(paced(120, 3000 / 120), 3000 / 120, 1e-9);
  near(paced(60, 1000 / 30), 2000 / 60, 1e-9);
  // No whole number of 144 Hz refreshes makes 30 Hz, so the pace is the first
  // multiple that reaches it, not the last one short of it.
  near(paced(144, 1000 / 30), 5000 / 144, 1e-9);
  // A refresh measured a hair long does not add a refresh.
  near(paced(1000 / 8.34, 3 * 8.33), 3 * 8.34, 1e-9);
});

test("frame pacing - an interval under the display's own moves nothing, and 0 asks none", () => {
  const q = new FramePacing(POLICY, () => {});
  q.pace(1);
  near(grant_after_gaps(q, 1000 / 120).paced_ms, 2000 / 120, 1e-9);
  q.pace(1000 / 30);
  near(q.grant(0).paced_ms, 4000 / 120, 1e-9);
  q.pace(0);
  near(q.grant(0).paced_ms, 2000 / 120, 1e-9);
});

test("frame pacing - an input hurries no turn past the interval asked", () => {
  const q = new FramePacing(POLICY, () => {});
  const refresh_ms = 1000 / 120;
  grant_after_gaps(q, refresh_ms);
  q.pace(1000 / 30);
  let now = 1000;
  assert.ok(q.admit(now));
  const admitted: number[] = [];
  for (let i = 1; i <= 12; i++) {
    now += refresh_ms;
    q.input(now);
    if (q.admit(now)) admitted.push(i);
  }
  assert.deepStrictEqual(admitted, [4, 8, 12]);
  assert.ok(q.grant(now).interactive);
});

test("frame pacing - a gap that is not a frame moves nothing", () => {
  const q = new FramePacing(POLICY, () => {});
  const before = grant_after_gaps(q, 1000 / 165);
  q.gap(3000);
  assert.deepStrictEqual(q.grant(0).refresh_ms, before.refresh_ms);
});

test("frame pacing - the deadline is the paced interval less the margin", () => {
  const q = new FramePacing(POLICY, () => {});
  const g = grant_after_gaps(q, 1000 / 120);
  assert.ok(g.interactive);
  near(g.deadline, g.paced_ms * (1 - POLICY.margin_share), 1e-9);
});

test("frame pacing - a turn that ran past its deadline owes the next nothing", () => {
  const q = new FramePacing(POLICY, () => {});
  const full = grant_after_gaps(q, 1000 / 30);
  q.observe(full.deadline + 3, full, 0);
  near(q.grant(0).deadline, full.deadline, 1e-9);
});

test("frame pacing - the deadline runs from the frame's start, so a late turn has the less", () => {
  const q = new FramePacing(POLICY, () => {});
  const full = grant_after_gaps(q, 1000 / 30);
  assert.ok(q.admit(1000));
  near(q.grant(1005).deadline, 1000 + full.deadline, 1e-9);
});

test("frame pacing - input, or a core asking for the next edge, makes a turn interactive", () => {
  const q = new FramePacing(POLICY, () => {});
  for (let i = 0; i < FRAME_WINDOW; i++) q.gap(1000 / 30);
  q.observe(0, q.grant(0), NO_WAKE);
  const idle = q.grant(0);
  assert.ok(!idle.interactive);
  near(
    idle.deadline,
    idle.paced_ms * POLICY.batch_frames * (1 - POLICY.margin_share),
    1e-9,
  );
  q.input(1000);
  assert.ok(q.grant(1000 + POLICY.settle_ms - 1).interactive);
  assert.ok(!q.grant(1000 + POLICY.settle_ms).interactive);
  q.observe(0, q.grant(0), 0);
  const asked = q.grant(0);
  assert.ok(asked.interactive);
  near(asked.deadline, asked.paced_ms * (1 - POLICY.margin_share), 1e-9);
});
