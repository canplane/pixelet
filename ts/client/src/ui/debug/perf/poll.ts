// @pxl/client/ui/debug/perf/poll.ts

/**
 * The one poll of the profiler the debug panels read: while the profiler
 * panel is up, the gauge over the live window, whose view the view panel
 * reads too; while only the view panel is open, the view alone
 * (port/perf.ts `read("view")`); while neither, nothing.
 */

import { useEffect } from "react";
import { create } from "zustand";

import { read as perf_read } from "@/port/perf.ts";
import { type Gauge, perfGaugeRead } from "./view.ts";

const POLL_MS = 500;

export const LIVE_WINDOW_MS = 10_000;

const store = create<{ gauge: Gauge | null; view: Gauge["view"] | null }>()(
  () => ({ gauge: null, view: null }),
);

/** The panels reading each, which says what a poll reads. */
const readers = { gauge: 0, view: 0 };
let timer: ReturnType<typeof setInterval> | undefined;

function read(): void {
  if (readers.gauge > 0) {
    void perfGaugeRead(LIVE_WINDOW_MS).then((gauge) =>
      store.setState({ gauge, view: gauge.view }),
    );
  } else if (readers.view > 0) {
    void perf_read("view").then((view) => store.setState({ view }));
  }
}

/** Read at once, then every POLL_MS while anyone reads; the returned call stops reading. */
function join(what: keyof typeof readers): () => void {
  readers[what]++;
  read();
  timer ??= setInterval(read, POLL_MS);
  return () => {
    readers[what]--;
    // A panel shown again starts from nothing, not from what it read before.
    if (readers.gauge === 0) store.setState({ gauge: null });
    if (readers.view === 0 && readers.gauge === 0) {
      store.setState({ view: null });
    }
    if (readers.gauge + readers.view === 0) {
      clearInterval(timer);
      timer = undefined;
    }
  };
}

/** The gauge over the live window, polled while `on`; null until the first read. */
export function usePerfGauge(on: boolean): Gauge | null {
  useEffect(() => (on ? join("gauge") : undefined), [on]);
  return store((s) => (on ? s.gauge : null));
}

/** Where the camera and the pointer are now, polled while `on`; null until the first read. */
export function usePerfView(on: boolean): Gauge["view"] | null {
  useEffect(() => (on ? join("view") : undefined), [on]);
  return store((s) => (on ? s.view : null));
}
