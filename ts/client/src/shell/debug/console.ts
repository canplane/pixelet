// @pxl/client/shell/debug/console.ts

/**
 * A development page's console: `xpute`, the guest's doorbell, whose `cmd`
 * sends the kernel any command by its number (xpute host ipc/doorbell.ts
 * `Doorbell.cmd`); and `pxl.profiler`, what the profiler panel exports, made
 * on this side of the door: `report()`, the window's summary as text, and
 * `trace()`, the window saved for ui.perfetto.dev.
 */

import { kernel_doorbell } from "@/port/shell.ts";
import { profilerReportText } from "./profiler/report.ts";
import { profilerTraceSave } from "./profiler/trace.ts";
import { profilerReportRead } from "./profiler/view.ts";

/** Once the kernel has booted: its doorbell is made then. */
export function installConsole(): void {
  const g = globalThis as { xpute?: unknown; pxl?: unknown };
  g.xpute = kernel_doorbell();
  g.pxl = {
    profiler: {
      report: () => profilerReportText(profilerReportRead()),
      trace: profilerTraceSave,
    },
  };
}
