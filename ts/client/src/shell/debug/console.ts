// @pxl/client/shell/debug/console.ts

/**
 * A development page's console: `xpute`, whose `cmd` sends the kernel any
 * command by its number, as the shell (port/shell.ts `kernel_console`); and
 * `pxl.profiler`, what the profiler panel exports, made on this side of the
 * door: `report()`, the window's summary as text, `trace()`, the window saved
 * for ui.perfetto.dev, and `drive(motion, seconds, hour)`, a window taken the
 * same way each time (profiler/drive.ts).
 */

import { kernel_console } from "@/port/shell.ts";
import { profilerDrive } from "./profiler/drive.ts";
import { profilerReportText } from "./profiler/report.ts";
import { profilerTraceSave } from "./profiler/trace.ts";
import { profilerReportRead } from "./profiler/view.ts";

/** Once the kernel has booted: its doorbell is made then. */
export function installConsole(): void {
  const g = globalThis as { xpute?: unknown; pxl?: unknown };
  g.xpute = kernel_console;
  g.pxl = {
    profiler: {
      report: () => profilerReportText(profilerReportRead()),
      trace: profilerTraceSave,
      drive: profilerDrive,
    },
  };
}
