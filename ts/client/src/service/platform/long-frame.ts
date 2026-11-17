// @pxl/client/service/platform/long-frame.ts

/** A place reports long animation frames, which Safari, WKWebView and React
 * Native lack, and long tasks. Natively, the long tasks are the JavaScript
 * thread's, which React Native's native performance API reports
 * (RuntimeScheduler_Modern.cpp `reportLongTasks`). Both platforms read the
 * same performance API, so it is one file and no pair: a pair keeps only what
 * differs. */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { LongFrame, LongFrameSource } from "./types.ts";

interface LoafEntry extends PerformanceEntry {
  blockingDuration?: f64;
  renderStart: f64;
  scripts?: { duration: f64 }[];
}

const ENTRY: Record<LongFrameSource, string> = {
  loaf: "long-animation-frame",
  longtask: "longtask",
};

/** A long task blocks for its time past 50 ms, the length at which the
 * browser and React Native call a task long. */
const LONG_TASK_MS = 50;

const supports = (source: LongFrameSource): boolean =>
  typeof PerformanceObserver !== "undefined" &&
  (PerformanceObserver.supportedEntryTypes?.includes(ENTRY[source]) ?? false);

export function sources(): readonly LongFrameSource[] {
  return (["loaf", "longtask"] as const).filter(supports);
}

export function watch(heard: (frame: LongFrame) => void): void {
  const has = sources();
  if (has.includes("loaf")) {
    new PerformanceObserver((list) => {
      for (const raw of list.getEntries()) {
        const e = raw as LoafEntry;
        const script_ms =
          e.scripts?.reduce((sum, s) => sum + s.duration, 0) ?? 0;
        const render_ms =
          e.renderStart > 0
            ? Math.max(0, e.startTime + e.duration - e.renderStart)
            : 0;
        heard({
          source: "loaf",
          t: e.startTime,
          duration_ms: e.duration,
          blocking_ms: e.blockingDuration ?? 0,
          script_ms,
          render_ms,
        });
      }
    }).observe({ type: ENTRY.loaf, buffered: true });
  }
  if (has.includes("longtask")) {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        heard({
          source: "longtask",
          t: e.startTime,
          duration_ms: e.duration,
          blocking_ms: Math.max(0, e.duration - LONG_TASK_MS),
          script_ms: 0,
          render_ms: 0,
        });
      }
    }).observe({ type: ENTRY.longtask, buffered: true });
  }
}
