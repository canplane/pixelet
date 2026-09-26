// @pxl/web/feature/debug/ledger/long-frame.ts
//
// The long-frame observers, and the ring they fill. A frame that blocks the
// event loop past 50ms is a browser measurement of the whole task, and the
// kernel's tick is only part of that task — so it is measured here, and it
// stays here: the ledger is read-only and nothing in the kernel would do
// anything with it, so it does not cross. The panel and the report put these
// beside the kernel's frames by time, both clocks being `performance.now()`.
//
// Both observers feed the same ring and each record says which it came from.
// LoAF is the richer one (a script/render split and the browser's own
// blocking duration) and is Chromium-only; `longtask` reports a duration and
// nothing else but fires everywhere, and a >50ms task is exactly the event
// worth catching. Installed alongside each other rather than instead.
//
// A debug instrument, installed with the ledger's panel rather than at boot:
// nothing that always runs can lean on LoAF, which is Chromium's alone.

import type { f64 } from "@xpute/core/abi/word.ts";

/** One long animation frame (or long task, where LoAF is missing). */
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

/** As long as the kernel's rings keep: the last minute, and at most this many. */
const WINDOW_MS = 60_000;
const LIMIT = 1_024;

const ring: LongFrame[] = [];
let sink: ((l: LongFrame) => void) | null = null;

function land(l: LongFrame): void {
  const cutoff = l.t - WINDOW_MS;
  let drop = 0;
  while (drop < ring.length && ring[drop].t < cutoff) drop++;
  if (drop > 0) ring.splice(0, drop);
  if (ring.length === LIMIT) ring.shift();
  ring.push(l);
  sink?.(l);
}

/** Long frames starting at or after `t`. Filtered rather than searched: two
 * observers feed this ring, so it is not strictly in time order. */
export function longFramesSince(t: f64): LongFrame[] {
  return ring.filter((l) => l.t >= t);
}

/** Forgets every long frame: a fresh window. */
export function longFramesClear(): void {
  ring.length = 0;
}

/** Hands each long frame, as it lands, to the live stream; none turns it off. */
export function setLongFrameSink(to: ((l: LongFrame) => void) | null): void {
  sink = to;
}

function supports(type: string): boolean {
  return typeof PerformanceObserver !== "undefined" && (PerformanceObserver.supportedEntryTypes?.includes(type) ?? false);
}

/** Whether this browser has long-animation-frame entries at all — Safari and
 * iOS WKWebView do not, and the report says so rather than reading as a
 * window with no long frames in it. */
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
