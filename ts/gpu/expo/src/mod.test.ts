// @pxl/gpu/expo/src/mod.test.ts

import { test, vi } from "vitest";
import assert from "node:assert/strict";

import type { GpuState } from "../../link/src/gpu.ts";

/** The GPU's native module as the host stands: the state now, and what each
 * listener is told as it changes. */
const module = vi.hoisted(() => ({
  state: null as GpuState | null,
  listeners: [] as ((e: { state: GpuState }) => void)[],
}));

vi.mock("expo", () => ({
  requireNativeModule: () => ({
    addListener: (_: string, h: (e: { state: GpuState }) => void) => {
      module.listeners.push(h);
      return { remove() {} };
    },
    state: () => module.state,
  }),
  requireNativeView: () => null,
}));

const { state_on } = await import("./mod.ts");

test("a UI that starts after the host told its state hears the state now, then each change", () => {
  const before: GpuState[] = [];
  state_on((s) => before.push(s));
  assert.deepEqual(before, [], "no state before the guest's first ask");

  module.state = "present";
  const late: GpuState[] = [];
  state_on((s) => late.push(s));
  module.state = "waiting";
  for (const h of module.listeners) h({ state: "waiting" });
  assert.deepEqual(late, ["present", "waiting"]);
});
