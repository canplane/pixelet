// @xpute/core/sched/tick.test.ts

import { assert, assertEquals, assertThrows } from "@std/assert";

import type { Grant } from "./quantum.ts";
import { CONTENT, COSMETIC, FrameBudget, INTERACTION, PHASES, REPORT, Tick, type TickPolicy, VISIBLE } from "./tick.ts";

function burn(ms: number): void {
  const end = performance.now() + ms;
  while (performance.now() < end) { /* spin */ }
}

type Seen = { order: string[]; a: number; b: number; runs: number; id: number; moving: boolean };

const seen = (over: Partial<Seen> = {}): Seen => ({ order: [], a: 0, b: 0, runs: 0, id: 0, moving: false, ...over });

const POLICY: TickPolicy = { settle_ms: 150, hard_slack_ms: 4, content_share: 0.2 };
const GRANT: Grant = { slice_ms: 6, batch_ms: 12, frame_ms: 1000 / 60 };

const tick = () => new Tick<Seen>(POLICY, 8);

Deno.test("tick - phases run in their fixed order, whatever order they registered in", () => {
  const t = tick();
  for (const phase of [REPORT, COSMETIC, CONTENT, VISIBLE, INTERACTION]) t.on_tick(phase, (c) => c.state.order.push(PHASES[phase]), "t");
  const s = seen();
  t.run(s, 0.016, GRANT);
  assertEquals(s.order, [...PHASES]);
});

Deno.test("tick - within a phase, steps run in registration order", () => {
  const t = tick();
  for (const n of ["1", "2", "3"]) t.on_tick(VISIBLE, (c) => c.state.order.push(n), "t");
  const s = seen();
  t.run(s, 0.016, GRANT);
  assertEquals(s.order, ["1", "2", "3"]);
});

Deno.test("tick - one budget is shared: what an earlier phase spends, a later one does not get", () => {
  const t = tick();
  t.on_tick(VISIBLE, (c) => burn(c.budget.total_ms * 0.75), "t");
  t.on_tick(CONTENT, (c) => void (c.state.a = c.budget.remaining()), "t");
  const s = seen({ a: -1 });
  const report = t.run(s, 0.016, GRANT);
  assert(s.a >= 0);
  assert(s.a <= report.budget_ms * 0.3, `content saw ${s.a} of ${report.budget_ms}`);
});

Deno.test("tick - the visible phase cannot take the content share, and content sees it", () => {
  const t = tick();
  t.on_tick(VISIBLE, (c) => {
    c.state.a = c.budget.remaining();
    // Runs the budget it was shown to the floor.
    c.budget.run(new Array(1000).fill(0), () => (burn(0.05), true));
  }, "t");
  t.on_tick(CONTENT, (c) => void (c.state.b = c.budget.remaining()), "t");
  const s = seen();
  const report = t.run(s, 0.016, GRANT);
  assert(s.a <= report.budget_ms * 0.8 + 0.01, `visible saw ${s.a} of ${report.budget_ms}`);
  assert(s.b >= report.budget_ms * 0.1, `content saw ${s.b} of ${report.budget_ms}`);
});

Deno.test("tick - a step that unregisters itself mid-tick is not run again", () => {
  const t = tick();
  const id = t.on_tick(COSMETIC, (c) => {
    c.state.runs++;
    c.off_tick(c.state.id);
  }, "t");
  const s = seen({ id });
  t.run(s, 0.016, GRANT);
  t.run(s, 0.016, GRANT);
  assertEquals(s.runs, 1);
});

Deno.test("tick - a step that unregisters an earlier one does not make the tick skip the step after it", () => {
  const t = tick();
  const first = t.on_tick(VISIBLE, (c) => c.state.order.push("first"), "t");
  t.on_tick(VISIBLE, (c) => {
    c.state.order.push("second");
    c.off_tick(c.state.id);
  }, "t");
  t.on_tick(VISIBLE, (c) => c.state.order.push("third"), "t");
  const s = seen({ id: first });
  t.run(s, 0.016, GRANT);
  assertEquals(s.order, ["first", "second", "third"]);
});

Deno.test("tick - the report phase sees every phase before it", () => {
  const t = tick();
  t.on_tick(VISIBLE, () => burn(2), "t");
  t.on_tick(REPORT, (c) => {
    c.state.a = c.phase_ms[VISIBLE];
    c.state.b = c.phase_ms[REPORT];
  }, "t");
  const s = seen({ a: -1 });
  t.run(s, 0.016, GRANT);
  assert(s.a >= 2, `visible ${s.a}`);
  assertEquals(s.b, 0);
});

Deno.test("frame_budget.run - forward progress survives an exhausted pool", () => {
  const budget = new FrameBudget(0, performance.now(), POLICY.hard_slack_ms);
  burn(1);
  const stats = budget.run([1, 2, 3], () => true);
  assertEquals(stats.ran, 1);
  assertEquals(stats.stopped_early, true);
});

const COST = 0.3;

Deno.test("tick - a storm of steps that report work stays within the budget plus one step", () => {
  const t = tick();
  const items = Array.from({ length: 2000 }, (_, i) => i);
  t.on_tick(VISIBLE, (c) => void (c.state.runs = c.budget.run(items, () => (burn(COST), true)).ran), "t");
  const s = seen();
  const report = t.run(s, 0.016, GRANT);
  assert(s.runs >= 1, "forward progress");
  assert(s.runs < items.length, "the storm was cut short");
  assert(report.phase_ms[VISIBLE] <= report.budget_ms + COST + 1, `visible ${report.phase_ms[VISIBLE]} over budget ${report.budget_ms}`);
});

Deno.test("tick - a storm of steps that cost time and report none stays within the hard ceiling", () => {
  const t = tick();
  const items = Array.from({ length: 2000 }, (_, i) => i);
  t.on_tick(VISIBLE, (c) => void c.budget.run(items, () => (burn(COST), false)), "t");
  const report = t.run(seen(), 0.016, GRANT);
  assert(report.phase_ms[VISIBLE] <= report.budget_ms + POLICY.hard_slack_ms + COST + 1, `visible ${report.phase_ms[VISIBLE]} over budget ${report.budget_ms}`);
});

Deno.test("tick - a view that has held still runs on the batch quantum, and loses it the turn it moves", async () => {
  const t = tick();
  t.on_tick(INTERACTION, (c) => {
    if (c.state.moving) c.view_moving();
  }, "t");
  const s = seen({ moving: true });
  const while_moving = t.run(s, 1 / 60, GRANT);
  s.moving = false;
  // Held still, but not yet for long enough: a pause inside a drag.
  const paused = t.run(s, 1 / 60, GRANT);
  await new Promise((resolve) => setTimeout(resolve, 200));
  const still = t.run(s, 1 / 60, GRANT);
  s.moving = true;
  const again = t.run(s, 1 / 60, GRANT);
  assert(!while_moving.settled && while_moving.budget_ms === GRANT.slice_ms);
  assert(!paused.settled);
  assert(still.settled && still.budget_ms === GRANT.batch_ms);
  assert(!again.settled && again.budget_ms === GRANT.slice_ms);
});

Deno.test("tick - what the interaction phase spends comes off the budget", () => {
  const t = tick();
  t.on_tick(INTERACTION, (c) => {
    c.view_moving();
    burn(1.5);
  }, "t");
  t.on_tick(VISIBLE, (c) => {
    c.state.a = c.budget.remaining();
    c.state.b = c.budget.total_ms;
  }, "t");
  const s = seen();
  t.run(s, 1 / 60, GRANT);
  assert(s.a <= s.b - 1.5, `visible saw ${s.a} of ${s.b}`);
});

Deno.test("tick - the step table is fixed: a step past it is refused", () => {
  const t = tick();
  assertThrows(() => {
    for (let i = 0; i < 9; i++) t.on_tick(COSMETIC, () => {}, "t");
  }, Error, "steps are taken");
});
