// @pxl/native/long-frame.ts

/** The app's long frames, by React Native's PerformanceObserver: long tasks on its JavaScript thread, where its
 * native performance API is (RuntimeScheduler_Modern.cpp `reportLongTasks`), and no long animation frames, which it has none of. */

import type { LongFrame, LongFrameSource } from "@pxl/client/bracket.tsx";

/** What a long task blocks past: the 50 ms React Native calls one long at. */
const LONG_TASK_MS = 50;

export function sources(): readonly LongFrameSource[] {
  const has = typeof PerformanceObserver !== "undefined" &&
    (PerformanceObserver.supportedEntryTypes?.includes("longtask") ?? false);
  return has ? ["longtask"] : [];
}

export function watch(heard: (frame: LongFrame) => void): void {
  if (sources().length === 0) return;
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
  }).observe({ type: "longtask", buffered: true });
}
