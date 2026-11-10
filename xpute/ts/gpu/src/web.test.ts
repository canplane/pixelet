// @xpute/gpu/web.test.ts

import { afterEach, expect, test, vi } from "vitest";

import type { GpuGrant } from "./gpu.ts";
import { WebGpu } from "./web.ts";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test("web - asking again waits for the page to be shown, and spends no try hidden", async () => {
  vi.useFakeTimers();
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "error").mockImplementation(() => {});
  const requestAdapter = vi.fn(() => Promise.resolve(null));
  vi.stubGlobal("navigator", { gpu: { requestAdapter } });
  let shown = false;
  const states: string[] = [];
  const gpu = new WebGpu();
  gpu.watch((state) => states.push(state));
  gpu.grant({
    ask: { power: "high-performance", features: {} },
    shown: () => shown,
  } as unknown as GpuGrant);

  await vi.advanceTimersByTimeAsync(60_000);
  expect(requestAdapter).toHaveBeenCalledTimes(1);
  expect(states).toEqual(["waiting"]);

  shown = true;
  await vi.advanceTimersByTimeAsync(1_000);
  expect(requestAdapter).toHaveBeenCalledTimes(2);
});
