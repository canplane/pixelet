// @xpute/runtime/sched/tick.test.ts

import { assert, assertEquals } from "@std/assert";

import { CONTENT, COSMETIC, FrameBudget, INTERACTION, PHASES, REPORT, Tick, type TickPolicy, VISIBLE } from "./tick.ts";
import { set_clock } from "../clock.ts";

/**
 * A test that owns the clock.
 *
 * These assert what a budget gives away and what it holds back, and a spin
 * loop answers that in wall time a shared runner does not owe them: a step
 * preempted mid-spin overshoots, and on CI one did — the phase after it saw
 * 0.18ms of the 0.6 the content share promises (2026-09-19). The scheduler
 * already reads time through clock.ts, so the tests move it by hand and the
 * arithmetic is exact rather than nearly.
 *
 * Installed per test and put back after, because frame_budget.ts reads the same clock
 * and its own tests want the real one.
 */
function test_on_a_held_clock(name: string, body: (clock: { now: () => number; burn: (ms: number) => void }) => void): void {
  Deno.test(name, () => {
    let t = 1_000;
    set_clock(() => t);
    try {
      body({ now: () => t, burn: (ms) => void (t += ms) });
    } finally {
      set_clock(() => performance.now());
    }
  });
}

type Seen = { order: string[]; a: number; b: number; runs: number; id: number };

const seen = (over: Partial<Seen> = {}): Seen => ({ order: [], a: 0, b: 0, runs: 0, id: 0, ...over });

const POLICY: TickPolicy = { hard_slack_ms: 4, content_share: 0.2 };
const QUOTA_MS = 6;

const tick = () => new Tick<Seen>(POLICY, 8);

test_on_a_held_clock("tick - phases run in their fixed order, whatever order they registered in", (clock) => {
  const t = tick();
  for (const phase of [REPORT, COSMETIC, CONTENT, VISIBLE, INTERACTION]) t.on_tick(phase, (c) => c.state.order.push(PHASES[phase]), "t");
  const s = seen();
  t.run(s, 0.016, QUOTA_MS, clock.now());
  assertEquals(s.order, [...PHASES]);
});

test_on_a_held_clock("tick - within a phase, steps run in registration order", (clock) => {
  const t = tick();
  for (const n of ["1", "2", "3"]) t.on_tick(VISIBLE, (c) => c.state.order.push(n), "t");
  const s = seen();
  t.run(s, 0.016, QUOTA_MS, clock.now());
  assertEquals(s.order, ["1", "2", "3"]);
});

test_on_a_held_clock("tick - one budget is shared: what an earlier phase spends, a later one does not get", (clock) => {
  const t = tick();
  t.on_tick(VISIBLE, (c) => clock.burn(c.budget.total_ms * 0.75), "t");
  t.on_tick(CONTENT, (c) => void (c.state.a = c.budget.remaining()), "t");
  const s = seen({ a: -1 });
  const report = t.run(s, 0.016, QUOTA_MS, clock.now());
  assert(s.a >= 0);
  assert(s.a <= report.budget_ms * 0.3, `content saw ${s.a} of ${report.budget_ms}`);
});

test_on_a_held_clock("tick - the visible phase cannot take the content share, and content sees it", (clock) => {
  const t = tick();
  t.on_tick(VISIBLE, (c) => {
    c.state.a = c.budget.remaining();
    // Runs the budget it was shown to the floor.
    c.budget.run(new Array(1000).fill(0), () => (clock.burn(0.05), true));
  }, "t");
  t.on_tick(CONTENT, (c) => void (c.state.b = c.budget.remaining()), "t");
  const s = seen();
  const report = t.run(s, 0.016, QUOTA_MS, clock.now());
  assert(s.a <= report.budget_ms * 0.8 + 0.01, `visible saw ${s.a} of ${report.budget_ms}`);
  assert(s.b >= report.budget_ms * 0.1, `content saw ${s.b} of ${report.budget_ms}`);
});

test_on_a_held_clock("tick - a step that unregisters itself mid-tick is not run again", (clock) => {
  const t = tick();
  const id = t.on_tick(COSMETIC, (c) => {
    c.state.runs++;
    c.off_tick(c.state.id);
  }, "t");
  const s = seen({ id });
  t.run(s, 0.016, QUOTA_MS, clock.now());
  t.run(s, 0.016, QUOTA_MS, clock.now());
  assertEquals(s.runs, 1);
});

test_on_a_held_clock("tick - a step that unregisters an earlier one does not make the tick skip the step after it", (clock) => {
  const t = tick();
  const first = t.on_tick(VISIBLE, (c) => c.state.order.push("first"), "t");
  t.on_tick(VISIBLE, (c) => {
    c.state.order.push("second");
    c.off_tick(c.state.id);
  }, "t");
  t.on_tick(VISIBLE, (c) => c.state.order.push("third"), "t");
  const s = seen({ id: first });
  t.run(s, 0.016, QUOTA_MS, clock.now());
  assertEquals(s.order, ["first", "second", "third"]);
});

test_on_a_held_clock("tick - the report phase sees every phase before it", (clock) => {
  const t = tick();
  t.on_tick(VISIBLE, () => clock.burn(2), "t");
  t.on_tick(REPORT, (c) => {
    c.state.a = c.phase_ms[VISIBLE];
    c.state.b = c.phase_ms[REPORT];
  }, "t");
  const s = seen({ a: -1 });
  t.run(s, 0.016, QUOTA_MS, clock.now());
  assert(s.a >= 2, `visible ${s.a}`);
  assertEquals(s.b, 0);
});

test_on_a_held_clock("frame_budget.run - forward progress survives an exhausted pool", (clock) => {
  const budget = new FrameBudget(0, clock.now(), POLICY.hard_slack_ms);
  clock.burn(1);
  const stats = budget.run([1, 2, 3], () => true);
  assertEquals(stats.ran, 1);
  assertEquals(stats.stopped_early, true);
});

const COST = 0.3;

test_on_a_held_clock("tick - a storm of steps that report work stays within the budget plus one step", (clock) => {
  const t = tick();
  const items = Array.from({ length: 2000 }, (_, i) => i);
  t.on_tick(VISIBLE, (c) => void (c.state.runs = c.budget.run(items, () => (clock.burn(COST), true)).ran), "t");
  const s = seen();
  const report = t.run(s, 0.016, QUOTA_MS, clock.now());
  assert(s.runs >= 1, "forward progress");
  assert(s.runs < items.length, "the storm was cut short");
  assert(report.phase_ms[VISIBLE] <= report.budget_ms + COST + 1, `visible ${report.phase_ms[VISIBLE]} over budget ${report.budget_ms}`);
});

test_on_a_held_clock("tick - a storm of steps that cost time and report none stays within the hard ceiling", (clock) => {
  const t = tick();
  const items = Array.from({ length: 2000 }, (_, i) => i);
  t.on_tick(VISIBLE, (c) => void c.budget.run(items, () => (clock.burn(COST), false)), "t");
  const report = t.run(seen(), 0.016, QUOTA_MS, clock.now());
  assert(report.phase_ms[VISIBLE] <= report.budget_ms + POLICY.hard_slack_ms + COST + 1, `visible ${report.phase_ms[VISIBLE]} over budget ${report.budget_ms}`);
});

test_on_a_held_clock("tick - what the turn spent before the tick comes off the budget", (clock) => {
  const t = tick();
  t.on_tick(VISIBLE, (c) => void (c.state.a = c.budget.remaining()), "t");
  const edge = clock.now();
  clock.burn(2);
  const s = seen();
  t.run(s, 1 / 60, QUOTA_MS, edge);
  assert(s.a <= QUOTA_MS * 0.8 - 2 + 0.01, `visible saw ${s.a}`);
});

test_on_a_held_clock("tick - what the interaction phase spends comes off the budget", (clock) => {
  const t = tick();
  t.on_tick(INTERACTION, () => clock.burn(1.5), "t");
  t.on_tick(VISIBLE, (c) => {
    c.state.a = c.budget.remaining();
    c.state.b = c.budget.total_ms;
  }, "t");
  const s = seen();
  t.run(s, 1 / 60, QUOTA_MS, clock.now());
  assert(s.a <= s.b - 1.5, `visible saw ${s.a} of ${s.b}`);
});
