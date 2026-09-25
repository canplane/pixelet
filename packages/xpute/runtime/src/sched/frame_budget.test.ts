// @xpute/runtime/sched/frame_budget.test.ts

import { assertEquals } from "@std/assert";

import { run_under_budget } from "./frame_budget.ts";

/** Burns real wall time — the budget is measured with the host's clock, so
 * a step has to actually take time for the deadline to mean anything. */
function burn(ms: number): void {
  const end = performance.now() + ms;
  while (performance.now() < end) { /* spin */ }
}

Deno.test("run_under_budget - runs everything when the budget is ample", () => {
  const seen: number[] = [];
  const stats = run_under_budget([1, 2, 3], (n) => (seen.push(n), true), { budget_ms: 1000 });

  assertEquals(seen, [1, 2, 3]);
  assertEquals(stats.ran, 3);
  assertEquals(stats.stopped_early, false);
});

Deno.test("run_under_budget - stops at the wall-time budget", () => {
  const seen: number[] = [];
  const stats = run_under_budget([1, 2, 3, 4, 5], (n) => {
    seen.push(n);
    burn(6);
    return true;
  }, { budget_ms: 10 });

  // one step fits inside 10ms, the second crosses it, the third is refused
  assertEquals(seen.length < 5, true);
  assertEquals(stats.stopped_early, true);
  assertEquals(stats.ran, seen.length);
});

Deno.test("run_under_budget - the first working step always runs, even past budget", () => {
  const seen: number[] = [];
  const stats = run_under_budget([1, 2, 3], (n) => {
    seen.push(n);
    burn(5);
    return true;
  }, { budget_ms: 0 });

  // budget 0 would refuse everything without the forward-progress rule
  assertEquals(seen, [1]);
  assertEquals(stats.ran, 1);
  assertEquals(stats.stopped_early, true);
});

Deno.test("run_under_budget - max_steps caps completed work", () => {
  const seen: number[] = [];
  const stats = run_under_budget([1, 2, 3, 4, 5], (n) => (seen.push(n), true), { budget_ms: 1000, max_steps: 2 });

  assertEquals(seen, [1, 2]);
  assertEquals(stats.ran, 2);
  assertEquals(stats.stopped_early, true);
});

Deno.test("run_under_budget - a no-op step neither counts nor arms the guard", () => {
  // Only the last item does work; a budget of 0 must still reach it, because
  // the guard is not armed until something has actually run.
  const seen: number[] = [];
  const stats = run_under_budget([1, 2, 3, 4], (n) => {
    seen.push(n);
    return n === 4;
  }, { budget_ms: 0, max_steps: 1 });

  assertEquals(seen, [1, 2, 3, 4]);
  assertEquals(stats.ran, 1);
  assertEquals(stats.visited, 4);
  assertEquals(stats.stopped_early, false);
});

Deno.test("run_under_budget - hard_budget_ms bounds a pass of expensive no-ops", () => {
  // The 4711ms frame this exists for: every step costs real time and reports
  // false, so the soft budget — waived until something completes — never
  // arms and the pass walks the whole list.
  const items = Array.from({ length: 40 }, (_, i) => i);
  const seen: number[] = [];
  const stats = run_under_budget(items, (n) => {
    seen.push(n);
    burn(3);
    return false;
  }, { budget_ms: 4, hard_budget_ms: 12 });

  assertEquals(stats.ran, 0);
  assertEquals(stats.stopped_early, true);
  assertEquals(seen.length < items.length, true);
});

Deno.test("run_under_budget - without a ceiling, expensive no-ops still walk the whole list", () => {
  // Locks in the behavior the ceiling is opt-in against, so it is clear
  // this is a deliberate choice per caller and not an accident.
  const items = Array.from({ length: 6 }, (_, i) => i);
  const seen: number[] = [];
  const stats = run_under_budget(items, (n) => (seen.push(n), burn(3), false), { budget_ms: 1 });

  assertEquals(seen.length, items.length);
  assertEquals(stats.stopped_early, false);
});

Deno.test("run_under_budget - the first item is attempted even under a zero ceiling", () => {
  const seen: number[] = [];
  const stats = run_under_budget([1, 2, 3], (n) => (seen.push(n), burn(2), true), { budget_ms: 0, hard_budget_ms: 0 });

  assertEquals(seen, [1]);
  assertEquals(stats.ran, 1);
});

Deno.test("run_under_budget - an empty list is a clean no-op", () => {
  const stats = run_under_budget([], () => true, { budget_ms: 10 });
  assertEquals(stats.ran, 0);
  assertEquals(stats.visited, 0);
  assertEquals(stats.stopped_early, false);
});
