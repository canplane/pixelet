// @pxl/web/feature/debug/profiler/log.ts

/**
 * The profiler's live console stream, pulled from the module's memory rather
 * than pushed on the ring, which keeps a debug feature off a control path
 * that asserts on failure.
 */

import { Command, encode_profiler_log } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command, kernel_room } from "@/kernel/shell.ts";
import { frameLine, longFrameLine, taskLine } from "./report.ts";
import { profilerLogTake } from "./view.ts";
import { setLongFrameSink } from "./long-frame.ts";

/** Batched: a drain every frame would be paced by what it measures. The
 * buffer is capped, so a slow reader loses the oldest. */
const DRAIN_MS = 500;

let drain: ReturnType<typeof setInterval> | null = null;

function drainLog(): void {
  const log = profilerLogTake();
  for (const t of log.tasks) console.log(`[profiler] ${taskLine(t)}`);
  for (const f of log.frames) console.log(`[profiler] ${frameLine(f, null)}`);
}

/** Called from the console (`__pxl_profiler_log`). */
export function profilerLog(on: boolean): void {
  kernel_command(Command.PROFILER_LOG, encode_profiler_log(kernel_room, on));
  setLongFrameSink(on ? (l) => console.log(`[profiler] ${longFrameLine(l)}`) : null);
  if (on && drain === null) drain = setInterval(drainLog, DRAIN_MS);
  if (!on && drain !== null) {
    clearInterval(drain);
    drain = null;
  }
}
