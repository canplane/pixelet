// @pxl/client/ui/debug/perf/poll.ts

/**
 * The one poll of the profiler that the debug panels share. While the profiler
 * panel is shown, it reads the gauge over the live window, and the view panel
 * takes its view from that gauge. While only the view panel is shown, it reads
 * the view alone (`read("view")` in service/core/perf.ts). While neither is
 * shown, it reads nothing.
 */

import { useEffect } from "react";
import { create } from "zustand";

import { read as perf_read } from "@/service/core/perf.ts";
import { type Gauge, perfGaugeRead } from "./view.ts";

const POLL_MS = 500;

export const LIVE_WINDOW_MS = 10_000;

const store = create<{ gauge: Gauge | null; view: Gauge["view"] | null }>()(
  () => ({ gauge: null, view: null }),
);

/** How many panels read the gauge, and how many the view. The counts decide
 * what a poll reads. */
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

/** Joins the poll as a reader of `what`. The poll reads at once, then every
 * POLL_MS while any reader remains. The returned function leaves the poll. */
function join(what: keyof typeof readers): () => void {
  readers[what]++;
  read();
  timer ??= setInterval(read, POLL_MS);
  return () => {
    readers[what]--;
    // A panel that is shown again starts empty, not with what it read before.
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

/** The gauge over the live window, polled while `on`. It is null until the
 * first read lands. */
export function usePerfGauge(on: boolean): Gauge | null {
  useEffect(() => (on ? join("gauge") : undefined), [on]);
  return store((s) => (on ? s.gauge : null));
}

/** Where the camera and the pointer are now, polled while `on`. It is null
 * until the first read lands. */
export function usePerfView(on: boolean): Gauge["view"] | null {
  useEffect(() => (on ? join("view") : undefined), [on]);
  return store((s) => (on ? s.view : null));
}
