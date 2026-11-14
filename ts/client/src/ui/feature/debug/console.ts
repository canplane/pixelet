// @pxl/client/ui/feature/debug/console.ts

/**
 * What every build puts on the console. `pxl.perf` holds what the profiler panel exports,
 * computed on the client from what the core's profiler holds. `report()` gives
 * the summary of the profiler's window as text, and `read()` gives the same
 * summary as values. `trace()` saves the window as a trace for
 * ui.perfetto.dev. `drive()` takes the one standard measurement, driven the same way each time (perf/drive.ts).
 */

import { perf_drive } from "./perf/drive.ts";
import { perf_report_text } from "./perf/report.ts";
import { perf_trace_save } from "./perf/trace.ts";
import { perf_report_read } from "./perf/view.ts";

/** Call this once the core has booted: the profiler `pxl.perf` reads is the
 * core's. */
export function install_console(): void {
  const g = globalThis as { pxl?: unknown };
  g.pxl = {
    perf: {
      report: async () => perf_report_text(await perf_report_read()),
      read: perf_report_read,
      trace: perf_trace_save,
      drive: perf_drive,
    },
  };
}
