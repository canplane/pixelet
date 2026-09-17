// @pxl/web/dev/ledger/log.ts

/**
 * The ledger's live console stream, read out of the module's memory rather
 * than received on the ring.
 *
 * The kernel keeps the rows (pxl_kernel/module/world/sched/log.rs) and this
 * comes for them; nothing is pushed. That is the shape every operating system
 * gives instrumentation — ftrace and perf mmap their own buffers, printk is
 * read through /dev/kmsg — and here it also keeps a debug feature away from a
 * control path whose failure policy is to assert. Long frames are the shell's
 * own and are printed as they land.
 *
 * The lines are the report's, formatter for formatter, so a task watched
 * landing reads exactly as the same task does in the report printed later.
 */

import { frame_line, long_frame_line, task_line } from "./report.ts";
import { ledger_log_take } from "./view.ts";
import { set_long_frame_sink } from "./long_frame.ts";

/** Prints whatever the stream has kept since the last call. */
export function ledger_drain_log(): void {
  const log = ledger_log_take();
  for (const t of log.tasks) console.log(`[ledger] ${task_line(t)}`);
  for (const f of log.frames) console.log(`[ledger] ${frame_line(f, null)}`);
}

/** Long frames go to the console as they land while the stream is on. */
export function ledger_log_long_frames(on: boolean): void {
  set_long_frame_sink(on ? (l) => console.log(`[ledger] ${long_frame_line(l)}`) : null);
}
