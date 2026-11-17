// @pxl/client/ui/feature/debug/perf/long-frame.ts
//
// Keeps the platform's long frames (service/platform/long-frame.ts) in one
// table, for the profiler to show beside the core's frames on one timeline: the
// host counts the core's clock as the UI thread's `performance.now()`.

import type { f64 } from "@xpute/kit/abi/word.ts";
import { type LongFrame } from "@/service/platform/types.ts";
import * as longFrame from "@/service/platform/long-frame.ts";
import { LIMIT } from "@/abi/perf.spec.ts";

const WINDOW_MS = LIMIT.WINDOW_MS;
const CAP = 1_024;

const table: LongFrame[] = [];

function land(l: LongFrame): void {
  const cutoff = l.t - WINDOW_MS;
  let drop = 0;
  while (drop < table.length && table[drop].t < cutoff) drop++;
  if (drop > 0) table.splice(0, drop);
  if (table.length === CAP) table.shift();
  table.push(l);
}

/** The table is filtered rather than searched, because its two sources feed
 * it out of time order. */
export function longFramesSince(t: f64): LongFrame[] {
  return table.filter((l) => l.t >= t);
}

export function longFramesClear(): void {
  table.length = 0;
}

let installed = false;

export function installLongFrameObservers(): void {
  if (installed) return;
  installed = true;
  longFrame.watch(land);
}
