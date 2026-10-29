// @pxl/client/shell/debug/profiler/long-frame.ts
//
// The place's long frames (bracket.tsx `longFrame`) kept in one ring, placed
// beside the kernel's frames by `performance.now()`.

import type { f64 } from "@xpute/kit/abi/word.ts";
import { bracket, type LongFrame } from "@/bracket.tsx";
import { LIMITS } from "@/abi/profiler.spec.ts";

// The kernel's rings' window, one number (profiler.json `LIMITS.WINDOW_MS`).
const WINDOW_MS = LIMITS.WINDOW_MS;
const LIMIT = 1_024;

const ring: LongFrame[] = [];

function land(l: LongFrame): void {
  const cutoff = l.t - WINDOW_MS;
  let drop = 0;
  while (drop < ring.length && ring[drop].t < cutoff) drop++;
  if (drop > 0) ring.splice(0, drop);
  if (ring.length === CAP) ring.shift();
  ring.push(l);
}

/** Filtered, not searched: two sources feed the ring out of order. */
export function longFramesSince(t: f64): LongFrame[] {
  return ring.filter((l) => l.t >= t);
}

export function longFramesClear(): void {
  ring.length = 0;
}

let installed = false;

/** Idempotent; a place that reports no long frames feeds the ring none. */
export function installLongFrameObservers(): void {
  if (installed) return;
  installed = true;
  bracket.longFrame.watch(land);
}
