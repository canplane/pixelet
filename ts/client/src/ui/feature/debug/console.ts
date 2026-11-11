// @pxl/client/ui/feature/debug/console.ts

/**
 * What every build puts on the console. `xpute.cmd(cmd, ...args)` sends
 * the core any command by its number, the way the UI sends one (`raw` in
 * service/core/raw.ts). `pxl.perf` holds what the profiler panel exports,
 * computed on the client from what the core's profiler holds. `report()` gives
 * the summary of the profiler's window as text, and `read()` gives the same
 * summary as values. `trace()` saves the window as a trace for
 * ui.perfetto.dev. `drive()` takes the one standard measurement, driven the same way each time (perf/drive.ts).
 */

import { perfDrive } from "./perf/drive.ts";
import { perfReportText } from "./perf/report.ts";
import { perfTraceSave } from "./perf/trace.ts";
import { perfReportRead } from "./perf/view.ts";
import { raw } from "@/service/core/raw.ts";

/** Call this once the core has booted: the connection `xpute.cmd` sends
 * through exists only from then. */
export function installConsole(): void {
  const g = globalThis as { xpute?: unknown; pxl?: unknown };
  g.xpute = raw;
  g.pxl = {
    perf: {
      report: async () => perfReportText(await perfReportRead()),
      read: perfReportRead,
      trace: perfTraceSave,
      drive: perfDrive,
    },
  };
}
