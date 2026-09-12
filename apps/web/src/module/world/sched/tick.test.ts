// @pxl/web/module/world/sched/tick.test.ts

import { assert, assertEquals } from "@std/assert";
import type { RootState } from "@react-three/fiber";

import { FrameBudget, on_tick, PHASES, run_tick } from "./tick.ts";

const state = {} as RootState;

function burn(ms: number): void {
  const end = performance.now() + ms;
  while (performance.now() < end) { /* spin */ }
}

Deno.test("tick - phases run in their fixed order, whatever order they registered in", () => {
  const seen: string[] = [];
  const offs = [...PHASES].reverse().map((phase) => on_tick(phase, () => seen.push(phase)));
  try {
    run_tick(state, 0.016);
    assertEquals(seen, [...PHASES]);
  } finally {
    offs.forEach((off) => off());
  }
});

Deno.test("tick - within a phase, steps run in registration order", () => {
  const seen: number[] = [];
  const offs = [1, 2, 3].map((n) => on_tick("visible", () => seen.push(n)));
  try {
    run_tick(state, 0.016);
    assertEquals(seen, [1, 2, 3]);
  } finally {
    offs.forEach((off) => off());
  }
});

Deno.test("tick - one budget is shared: what an earlier phase spends, a later one does not get", () => {
  const seen = { content_remaining: -1 };
  const offs = [
    on_tick("visible", ({ budget }) => burn(budget.total_ms * 0.75)),
    on_tick("content", ({ budget }) => {
      seen.content_remaining = budget.remaining();
    }),
  ];
  try {
    const report = run_tick(state, 0.016);
    assert(seen.content_remaining >= 0);
    assert(seen.content_remaining <= report.budget_ms * 0.3, `content saw ${seen.content_remaining} of ${report.budget_ms}`);
  } finally {
    offs.forEach((off) => off());
  }
});

Deno.test("tick - the visible phase cannot take the content share, and content sees it", () => {
  const seen = { visible_remaining: -1, content_remaining: -1 };
  const offs = [
    on_tick("visible", ({ budget }) => {
      seen.visible_remaining = budget.remaining();
      // Runs the budget it was shown to the floor.
      budget.run(new Array(1000).fill(0), () => (burn(0.05), true));
    }),
    on_tick("content", ({ budget }) => {
      seen.content_remaining = budget.remaining();
    }),
  ];
  try {
    const report = run_tick(state, 0.016);
    assert(seen.visible_remaining <= report.budget_ms * 0.8 + 0.01, `visible saw ${seen.visible_remaining} of ${report.budget_ms}`);
    assert(seen.content_remaining >= report.budget_ms * 0.1, `content saw ${seen.content_remaining} of ${report.budget_ms}`);
  } finally {
    offs.forEach((off) => off());
  }
});

Deno.test("tick - a step that unregisters itself mid-tick is not run again", () => {
  let runs = 0;
  const off = on_tick("cosmetic", () => {
    runs++;
    off();
  });
  run_tick(state, 0.016);
  run_tick(state, 0.016);
  assertEquals(runs, 1);
});

Deno.test("tick - the report phase sees every phase before it", () => {
  const seen: { phases: Record<string, number> | null } = { phases: null };
  const offs = [
    on_tick("visible", () => burn(2)),
    on_tick("report", ({ phase_ms }) => {
      seen.phases = { ...phase_ms };
    }),
  ];
  try {
    run_tick(state, 0.016);
    assert(seen.phases !== null);
    assert(seen.phases.visible >= 2, `visible ${seen.phases.visible}`);
    assertEquals(seen.phases.report, 0);
  } finally {
    offs.forEach((off) => off());
  }
});

Deno.test("FrameBudget.run - forward progress survives an exhausted pool", () => {
  const budget = new FrameBudget(0);
  burn(1);
  const stats = budget.run([1, 2, 3], () => true);
  assertEquals(stats.ran, 1);
  assertEquals(stats.stopped_early, true);
});

Deno.test("tick - a storm of steps that report work stays within the budget plus one step", () => {
  // Two thousand pending items at a third of a millisecond each would be
  // most of a second if nothing stopped them. The soft deadline must stop
  // them one step past the budget.
  const COST = 0.3;
  const items = Array.from({ length: 2000 }, (_, i) => i);
  let ran = 0;
  const off = on_tick("visible", ({ budget }) => {
    ran = budget.run(items, () => (burn(COST), true)).ran;
  });
  try {
    const report = run_tick(state, 0.016);
    assert(ran >= 1, "forward progress");
    assert(ran < items.length, "the storm was cut short");
    assert(report.phase_ms.visible <= report.budget_ms + COST + 1, `visible ${report.phase_ms.visible} over budget ${report.budget_ms}`);
  } finally {
    off();
  }
});

Deno.test("tick - a storm of steps that cost time and report none stays within the hard ceiling", () => {
  // The case the hard ceiling exists for: steps that spend real time and
  // report nothing done, so the soft deadline — waived until something
  // completes — never arms. frame_budget.ts records the 4.7 s frame this
  // produced before there was a ceiling.
  const COST = 0.3;
  const items = Array.from({ length: 2000 }, (_, i) => i);
  const off = on_tick("visible", ({ budget }) => {
    budget.run(items, () => (burn(COST), false));
  });
  try {
    const report = run_tick(state, 0.016);
    // HARD_SLACK_MS is 4 over the pool.
    assert(report.phase_ms.visible <= report.budget_ms + 4 + COST + 1, `visible ${report.phase_ms.visible} over budget ${report.budget_ms}`);
  } finally {
    off();
  }
});

Deno.test("tick - the budget follows the display's measured frame, not an assumed 60 Hz", () => {
  // Only a moving view is sized from the frame; a still one gets the
  // settled budget whatever the display.
  const off = on_tick("interaction", ({ view_moving }) => view_moving());
  // 64 frames fill the whole measurement window, so each run starts clean.
  const at = (delta_s: number) => {
    let r = run_tick(state, delta_s);
    for (let i = 1; i < 64; i++) r = run_tick(state, delta_s);
    return r.budget_ms;
  };
  try {
    const at_60 = at(1 / 60);
    const at_165 = at(1 / 165);
    assert(at_165 < at_60, `165 Hz got ${at_165} ms, 60 Hz got ${at_60} ms`);
    // An idle gap under frameloop="demand" is not a frame and moves nothing.
    const idle = run_tick(state, 3).budget_ms;
    assertEquals(idle, at_165);
    at(1 / 60); // leave the window as the other tests expect it
  } finally {
    off();
  }
});

Deno.test("tick - a view that has held still gets the larger budget, and loses it the frame it moves", async () => {
  let moving = true;
  const off = on_tick("interaction", ({ view_moving }) => {
    if (moving) view_moving();
  });
  try {
    const while_moving = run_tick(state, 1 / 60);
    assert(!while_moving.settled);
    moving = false;
    // Held still, but not yet for long enough: a pause inside a drag.
    assert(!run_tick(state, 1 / 60).settled);
    await new Promise((resolve) => setTimeout(resolve, 200));
    const still = run_tick(state, 1 / 60);
    assert(still.settled);
    assert(still.budget_ms > while_moving.budget_ms, `still ${still.budget_ms} vs moving ${while_moving.budget_ms}`);
    // The frame the view moves again is sized for motion: the budget is
    // decided after the interaction phase, not before it.
    moving = true;
    const again = run_tick(state, 1 / 60);
    assert(!again.settled);
    assertEquals(again.budget_ms, while_moving.budget_ms);
  } finally {
    off();
  }
});

Deno.test("tick - what the interaction phase spends comes off the budget", () => {
  const seen = { remaining: -1, total: -1 };
  const offs = [
    on_tick("interaction", ({ view_moving }) => {
      view_moving();
      burn(1.5);
    }),
    on_tick("visible", ({ budget }) => {
      seen.remaining = budget.remaining();
      seen.total = budget.total_ms;
    }),
  ];
  try {
    run_tick(state, 1 / 60);
    assert(seen.remaining <= seen.total - 1.5, `visible saw ${seen.remaining} of ${seen.total}`);
  } finally {
    offs.forEach((off) => off());
  }
});

Deno.test("tick - a still view's frame gaps do not teach the tick a faster display", async () => {
  let moving = true;
  const off = on_tick("interaction", ({ view_moving }) => {
    if (moving) view_moving();
  });
  try {
    let moving_report = run_tick(state, 1 / 165);
    for (let i = 1; i < 64; i++) moving_report = run_tick(state, 1 / 165);
    moving = false;
    await new Promise((resolve) => setTimeout(resolve, 200));
    // The browser catching up after a long still frame: gaps far shorter
    // than any real refresh interval.
    let still_report = run_tick(state, 0.002);
    for (let i = 1; i < 64; i++) still_report = run_tick(state, 0.002);
    assert(still_report.settled);
    assertEquals(still_report.frame_ms, moving_report.frame_ms);
  } finally {
    off();
  }
});
