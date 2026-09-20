// @pxl/web/feature/dev/ledger/log.ts

/**
 * The ledger's live console stream, read out of the module's memory rather
 * than received on the ring.
 *
 * The kernel keeps the rows (crates/pxl/web/kernel/src/sched/ledger/log.rs) and this
 * comes for them; nothing is pushed. That is the shape every operating system
 * gives instrumentation — ftrace and perf mmap their own buffers, printk is
 * read through /dev/kmsg — and here it also keeps a debug feature away from a
 * control path whose failure policy is to assert. Long frames are the shell's
 * own and are printed as they land.
 *
 * The lines are the report's, formatter for formatter, so a task watched
 * landing reads exactly as the same task does in the report printed later.
 */

import { frameLine, longFrameLine, taskLine } from "./report.ts";
import { ledgerLogTake } from "./view.ts";
import { setLongFrameSink } from "./long-frame.ts";

/** Prints whatever the stream has kept since the last call. */
export function ledgerDrainLog(): void {
  const log = ledgerLogTake();
  for (const t of log.tasks) console.log(`[ledger] ${taskLine(t)}`);
  for (const f of log.frames) console.log(`[ledger] ${frameLine(f, null)}`);
}

/** Long frames go to the console as they land while the stream is on. */
export function ledgerLogLongFrames(on: boolean): void {
  setLongFrameSink(on ? (l) => console.log(`[ledger] ${longFrameLine(l)}`) : null);
}
