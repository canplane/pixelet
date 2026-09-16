// @pxl/web/module/world/sched/ledger_log.ts

/**
 * The ledger's live console stream, read out of the module's memory rather
 * than received on the ring.
 *
 * The kernel keeps the records (pxl_core/module/world/sched/log.rs) and this
 * comes for them; nothing is pushed. That is the shape every operating system
 * gives instrumentation — ftrace and perf mmap their own buffers, printk is
 * read through /dev/kmsg — and here it also keeps a debug feature away from a
 * control path whose failure policy is to assert.
 *
 * The lines are the report's, formatter for formatter, so a task watched
 * landing reads exactly as the same task does in the report printed later.
 */

import { CORE_KERNEL } from "@/wasm/core_memory.ts";
import { type Frame, frame_line, type LongFrame, long_frame_line, type TaskRecord, task_line } from "./ledger_report.ts";

type LedgerLog = { tasks: TaskRecord[]; frames: Frame[]; longs: LongFrame[] };

/** Prints whatever the stream has kept since the last call. */
export function ledger_drain_log(): void {
  const at = CORE_KERNEL.ledger_log();
  const [json_at, bytes] = new Uint32Array(CORE_KERNEL.memory.buffer, at, 2);
  if (bytes === 0) return;
  const json = new TextDecoder().decode(new Uint8Array(CORE_KERNEL.memory.buffer, json_at, bytes));
  CORE_KERNEL.ledger_log_release();
  const log = JSON.parse(json) as LedgerLog;
  for (const t of log.tasks) console.log(`[ledger] ${task_line(t)}`);
  for (const f of log.frames) console.log(`[ledger] ${frame_line(f, null)}`);
  for (const l of log.longs) console.log(`[ledger] ${long_frame_line(l)}`);
}
