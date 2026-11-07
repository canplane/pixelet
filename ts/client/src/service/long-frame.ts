// @pxl/client/service/long-frame.ts

/** The place's long frames, by its PerformanceObserver: long animation frames, which Safari, WKWebView and React Native lack, and long tasks,
 * on a device its JavaScript thread's, where its native performance API is (RuntimeScheduler_Modern.cpp `reportLongTasks`). */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { LongFrame, LongFrameSource } from "./types.ts";

interface LoafEntry extends PerformanceEntry {
  blockingDuration?: f64;
  renderStart: f64;
  scripts?: { duration: f64 }[];
}

/** The entry type each source is observed as. */
const ENTRY: Record<LongFrameSource, string> = {
  loaf: "long-animation-frame",
  longtask: "longtask",
};

/** What a long task blocks past: the 50 ms the browser and React Native call one long at. */
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
        const scriptMs =
          e.scripts?.reduce((sum, s) => sum + s.duration, 0) ?? 0;
        const renderMs =
          e.renderStart > 0
            ? Math.max(0, e.startTime + e.duration - e.renderStart)
            : 0;
        heard({
          source: "loaf",
          t: e.startTime,
          durationMs: e.duration,
          blockingMs: e.blockingDuration ?? 0,
          scriptMs,
          renderMs,
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
          durationMs: e.duration,
          blockingMs: Math.max(0, e.duration - LONG_TASK_MS),
          scriptMs: 0,
          renderMs: 0,
        });
      }
    }).observe({ type: ENTRY.longtask, buffered: true });
  }
}
