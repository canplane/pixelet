// @pxl/client/ui/debug/console.ts

/**
 * What a development build puts on the console. `xpute.cmd(cmd, ...args)`
 * sends the core any command by its number, the way the UI sends one (`door`
 * in port/ui.ts). `pxl.perf` holds what the profiler panel exports, computed
 * on the client from what the core's profiler holds. `report()` gives the
 * summary of the profiler's window as text, and `read()` gives the same
 * summary as values, which script/compare-perf.py reads. `trace()` saves the
 * window as a trace for ui.perfetto.dev. `drive(seconds)` takes the one
 * standard measurement, driven the same way each time (perf/drive.ts).
 */

import { perfDrive } from "./perf/drive.ts";
import { perfReportText } from "./perf/report.ts";
import { perfTraceSave } from "./perf/trace.ts";
import { perfReportRead } from "./perf/view.ts";
import * as ui from "@/port/ui.ts";

/** Call this once the core has booted: the doorbell that `xpute.cmd` rings
 * exists only from then. */
export function installConsole(): void {
  const g = globalThis as { xpute?: unknown; pxl?: unknown };
  g.xpute = ui.door;
  g.pxl = {
    perf: {
      report: async () => perfReportText(await perfReportRead()),
      read: perfReportRead,
      trace: perfTraceSave,
      drive: perfDrive,
    },
  };
}
