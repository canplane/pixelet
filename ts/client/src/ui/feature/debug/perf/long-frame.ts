// @pxl/client/ui/feature/debug/perf/long-frame.ts
//
// Keeps the platform's long frames (service/platform/long-frame.ts) in one
// ring, for the profiler to show beside the core's frames on one timeline: the
// host counts the core's clock as the UI thread's `performance.now()`.

import type { f64 } from "@xpute/kit/abi/word.ts";
import { type LongFrame } from "@/service/platform/types.ts";
import * as long_frame from "@/service/platform/long-frame.ts";
import { LIMIT } from "@/abi/perf.spec.ts";

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
export function long_frames_since(t: f64): LongFrame[] {
  return ring.filter((l) => l.t >= t);
}

export function long_frames_clear(): void {
  ring.length = 0;
}

let installed = false;

export function install_long_frame_observers(): void {
  if (installed) return;
  installed = true;
  long_frame.watch(land);
}
