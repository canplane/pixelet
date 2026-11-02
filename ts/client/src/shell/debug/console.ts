// @pxl/client/shell/debug/console.ts

/**
 * A development page's console: `xpute`, whose `cmd` sends the kernel any
 * command by its number, as the shell (port/shell.ts `kernel_console`); and
 * `pxl.perf`, what the profiler panel exports, made on this side of the
 * door: `report()`, the window's summary as text, `read()`, the same as
 * values (script/compare-perf.py reads it), `trace()`, the window saved
 * for ui.perfetto.dev, and `drive(motion, seconds, hour)`, a window taken the
 * same way each time (perf/drive.ts).
 */

import { kernel_console } from "@/port/shell.ts";
import { perfDrive } from "./perf/drive.ts";
import { perfReportText } from "./perf/report.ts";
import { perfTraceSave } from "./perf/trace.ts";
import { perfReportRead } from "./perf/view.ts";

/** Once the kernel has booted: its doorbell is made then. */
export function installConsole(): void {
  const g = globalThis as { xpute?: unknown; pxl?: unknown };
  g.xpute = kernel_console;
  g.pxl = {
    perf: {
      report: () => perfReportText(perfReportRead()),
      read: perfReportRead,
      trace: perfTraceSave,
      drive: perfDrive,
    },
  };
}
