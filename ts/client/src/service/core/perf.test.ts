// @pxl/client/service/core/perf.test.ts

import { test } from "vitest";
import assert from "node:assert/strict";

import { FRAME } from "@/abi/perf.spec.ts";
import { GPU_SPAN_NAMES } from "@/abi/span.spec.ts";
import { frameAt } from "./perf.ts";

test("frameAt - reads each lane at its word", () => {
  const words = Float64Array.from({ length: FRAME.WORDS }, (_, k) => k);
  const f = frameAt(words, 0);
  assert.deepStrictEqual(f.budget_ms, FRAME.budget_ms);
  assert.deepStrictEqual(f.gpu_spans[2], FRAME.gpu_spans + 2);
  assert.deepStrictEqual(f.gpu_spans.length, GPU_SPAN_NAMES.length);
  assert.deepStrictEqual(f.heap_used_bytes, FRAME.heap_used_bytes);
});
