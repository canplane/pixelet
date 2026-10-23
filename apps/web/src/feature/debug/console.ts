// @pxl/web/feature/debug/console.ts

/**
 * A development page's console: `xpute`, the guest's doorbell, whose `cmd`
 * sends the kernel any command by its number (xpute host ipc/doorbell.ts
 * `Doorbell.cmd`); and `pxl`, the page's one clock (`pxl.clock(unix_s)`, the
 * wall's again with none) and the profiler's reads (`pxl.gauge(ms)`,
 * `pxl.trace()`, `pxl.log(on)`). A check is typed here, not carried in the
 * page's URL.
 */

import { set_clock } from "@/kernel/sched.ts";
import { GUEST_DOORBELL } from "@/kernel/mem.ts";
import { profilerLog } from "./profiler/log.ts";
import { profilerTraceSave } from "./profiler/trace.ts";
import { profilerGaugeRead } from "./profiler/view.ts";

/** Once the kernel has booted: its doorbell is made then. */
export function installConsole(): void {
  const g = globalThis as { xpute?: unknown; pxl?: unknown };
  g.xpute = GUEST_DOORBELL;
  g.pxl = {
    clock: (unix_s?: number) => set_clock(unix_s === undefined ? () => Date.now() : () => unix_s * 1000),
    gauge: profilerGaugeRead,
    trace: profilerTraceSave,
    log: profilerLog,
  };
}
