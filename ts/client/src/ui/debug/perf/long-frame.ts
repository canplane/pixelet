// @pxl/client/ui/debug/perf/long-frame.ts
//
// Keeps the platform's long frames (service/long-frame.ts) in one ring, for
// the profiler to show beside the core's frames. Both are timed on the clock
// of `performance.now()`, so they line up.

import type { f64 } from "@xpute/kit/abi/word.ts";
import { type LongFrame } from "@/service/types.ts";
import * as long_frame from "@/service/long-frame.ts";
import { LIMIT } from "@/abi/perf.spec.ts";

// The ring spans the same window as the core's rings (perf.json
// `LIMIT.WINDOW_MS`).
const WINDOW_MS = LIMIT.WINDOW_MS;
const CAP = 1_024;

const ring: LongFrame[] = [];

function land(l: LongFrame): void {
  const cutoff = l.t - WINDOW_MS;
  let drop = 0;
  while (drop < ring.length && ring[drop].t < cutoff) drop++;
  if (drop > 0) ring.splice(0, drop);
  if (ring.length === CAP) ring.shift();
  ring.push(l);
}

/** The ring is filtered rather than searched, because its two sources feed
 * it out of time order. */
export function longFramesSince(t: f64): LongFrame[] {
  return ring.filter((l) => l.t >= t);
}

export function longFramesClear(): void {
  ring.length = 0;
}

let installed = false;

/** A second call does nothing. On a platform that reports no long frames,
 * the ring stays empty. */
export function installLongFrameObservers(): void {
  if (installed) return;
  installed = true;
  long_frame.watch(land);
}
