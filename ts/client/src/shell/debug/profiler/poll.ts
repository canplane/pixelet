// @pxl/client/shell/debug/profiler/poll.ts

/**
 * The one poll of the profiler the debug panels read: while the profiler
 * panel is up, the gauge over the live window, whose view the view panel
 * reads too; while only the view panel is open, the view alone
 * (abi/profiler.ts `profiler_view`); while neither, nothing.
 */

import { useEffect } from "react";
import { create } from "zustand";

import { profiler_view } from "@/abi/profiler.ts";
import { type Gauge, profilerGaugeRead } from "./view.ts";

const POLL_MS = 500;

/** Folding every frame since load on each repaint grew into a 60 ms task. */
export const LIVE_WINDOW_MS = 10_000;

const useRead = create<{ gauge: Gauge | null; view: Gauge["view"] | null }>(() => ({ gauge: null, view: null }));

/** The panels reading each, which says what a poll reads. */
const readers = { gauge: 0, view: 0 };
let timer: ReturnType<typeof setInterval> | undefined;

function read(): void {
  if (readers.gauge > 0) {
    const gauge = profilerGaugeRead(LIVE_WINDOW_MS);
    useRead.setState({ gauge, view: gauge.view });
  } else if (readers.view > 0) useRead.setState({ view: profiler_view() });
}

/** Read at once, then every POLL_MS while anyone reads; the returned call stops reading. */
function join(what: keyof typeof readers): () => void {
  readers[what]++;
  read();
  timer ??= setInterval(read, POLL_MS);
  return () => {
    readers[what]--;
    // A panel shown again starts from nothing, not from what it read before.
    if (readers.gauge === 0) useRead.setState({ gauge: null });
    if (readers.view === 0 && readers.gauge === 0) useRead.setState({ view: null });
    if (readers.gauge + readers.view === 0) {
      clearInterval(timer);
      timer = undefined;
    }
  };
}

/** The gauge over the live window, polled while `on`; null until the first read. */
export function useProfilerGauge(on: boolean): Gauge | null {
  useEffect(() => (on ? join("gauge") : undefined), [on]);
  return useRead((s) => (on ? s.gauge : null));
}

/** Where the camera and the pointer are now, polled while `on`; null until the first read. */
export function useProfilerView(on: boolean): Gauge["view"] | null {
  useEffect(() => (on ? join("view") : undefined), [on]);
  return useRead((s) => (on ? s.view : null));
}
