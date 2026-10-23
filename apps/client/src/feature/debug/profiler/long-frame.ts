// @pxl/client/feature/debug/profiler/long-frame.ts
//
// Long-frame observers (LoAF, Chromium-only, and `longtask` everywhere)
// feeding one ring, placed beside the kernel's frames by `performance.now()`.

import type { f64 } from "@xpute/kit/abi/word.ts";

export interface LongFrame {
  source: string;
  t: f64;
  durationMs: f64;
  blockingMs: f64;
  scriptMs: f64;
  renderMs: f64;
}

interface LoafEntry extends PerformanceEntry {
  blockingDuration?: f64;
  renderStart: f64;
  scripts?: { duration: f64 }[];
}

const WINDOW_MS = 60_000;
const LIMIT = 1_024;

const ring: LongFrame[] = [];

function land(l: LongFrame): void {
  const cutoff = l.t - WINDOW_MS;
  let drop = 0;
  while (drop < ring.length && ring[drop].t < cutoff) drop++;
  if (drop > 0) ring.splice(0, drop);
  if (ring.length === LIMIT) ring.shift();
  ring.push(l);
}

/** Filtered, not searched: two observers feed the ring out of order. */
export function longFramesSince(t: f64): LongFrame[] {
  return ring.filter((l) => l.t >= t);
}

export function longFramesClear(): void {
  ring.length = 0;
}

function supports(type: string): boolean {
  return typeof PerformanceObserver !== "undefined" && (PerformanceObserver.supportedEntryTypes?.includes(type) ?? false);
}

/** Safari and WKWebView have no LoAF; the report says so instead of showing none. */
export function loafSupported(): boolean {
  return supports("long-animation-frames");
}

let installed = false;

/** Idempotent, and a no-op for an observer this browser lacks. */
export function installLongFrameObservers(): void {
  if (installed) return;
  installed = true;

  if (loafSupported()) {
    new PerformanceObserver((list) => {
      for (const raw of list.getEntries()) {
        const e = raw as LoafEntry;
        const scriptMs = e.scripts?.reduce((sum, s) => sum + s.duration, 0) ?? 0;
        const renderMs = e.renderStart > 0 ? Math.max(0, e.startTime + e.duration - e.renderStart) : 0;
        land({ source: "loaf", t: e.startTime, durationMs: e.duration, blockingMs: e.blockingDuration ?? 0, scriptMs, renderMs });
      }
    }).observe({ type: "long-animation-frames", buffered: true } as PerformanceObserverInit);
  }

  if (supports("longtask")) {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        land({ source: "longtask", t: e.startTime, durationMs: e.duration, blockingMs: Math.max(0, e.duration - 50), scriptMs: 0, renderMs: 0 });
      }
    }).observe({ type: "longtask", buffered: true } as PerformanceObserverInit);
  }
}
