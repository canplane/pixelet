// @pxl/client/ui/debug/console.ts

/**
 * A development page's console: `xpute`, whose `cmd` sends the core any
 * command by its number, as the UI (port/ui.ts `ui.door`); and
 * `pxl.perf`, what the profiler panel exports, made on this side of the
 * door: `report()`, the window's summary as text, `read()`, the same as
 * values (script/compare-perf.py reads it), `trace()`, the window saved
 * for ui.perfetto.dev, and `drive(seconds)`, the one measurement, taken the
 * same way each time (perf/drive.ts).
 */

import { perfDrive } from "./perf/drive.ts";
import { perfReportText } from "./perf/report.ts";
import { perfTraceSave } from "./perf/trace.ts";
import { perfReportRead } from "./perf/view.ts";
import * as ui from "@/port/ui.ts";

/** Once the core has booted: its doorbell is made then. */
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
