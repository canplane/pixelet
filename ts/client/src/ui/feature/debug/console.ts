// @pxl/client/ui/feature/debug/console.ts

/**
 * What every build puts on the console. `pxl.perf` holds what the profiler panel exports,
 * computed on the client from what the core's profiler holds. `report()` gives
 * the summary of the profiler's window as text, and `read()` gives the same
 * summary as values. `trace()` saves the window as a trace for
 * ui.perfetto.dev. `drive()` takes the one standard measurement, driven the same way each time (perf/drive.ts).
 * `pxl.view.at(x, y)` hovers the pointer at a point of the view, in its
 * logical pixels, and gives as text what the drawn pixel there draws: the
 * words the GPU's ray left at it, read back (perf/view.ts `perfViewReadAt`), the
 * line `report()` shows for the pointer.
 */

import { perfDrive } from "./perf/drive.ts";
import { perfReportText, pointerText } from "./perf/report.ts";
import { perfTraceSave } from "./perf/trace.ts";
import { perfReportRead, perfViewReadAt } from "./perf/view.ts";

/** Call this once the core has booted: the profiler `pxl.perf` reads is the
 * core's. */
export function installConsole(): void {
  const g = globalThis as { pxl?: unknown };
  g.pxl = {
    perf: {
      report: async () => perfReportText(await perfReportRead()),
      read: perfReportRead,
      trace: perfTraceSave,
      drive: async () => perfReportText(await perfDrive()),
    },
    view: {
      at: async (x: number, y: number) => {
        const view = await perfViewReadAt(x, y);
        return view === null
          ? "  pointer no words landed: no world is drawn at that pixel"
          : pointerText(view);
      },
    },
  };
}
