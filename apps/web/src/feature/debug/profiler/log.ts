// @pxl/web/feature/debug/profiler/log.ts

/**
 * The profiler's live console stream, read out of the module's memory rather
 * than received on the ring.
 *
 * The kernel keeps the rows (crates/pxl/web/kernel/src/sched/profiler/log.rs) and this
 * comes for them; nothing is pushed. That is the shape every operating system
 * gives instrumentation — ftrace and perf mmap their own buffers, printk is
 * read through /dev/kmsg — and here it also keeps a debug feature away from a
 * control path whose failure policy is to assert. Long frames are the shell's
 * own and are printed as they land.
 *
 * The lines are the report's, formatter for formatter, so a task watched
 * landing reads exactly as the same task does in the report printed later.
 */

import { Command, encode_profiler_log } from "@/kernel/abi/cmd.spec.ts";
import { kernel_command } from "@/kernel/shell.ts";
import { frameLine, longFrameLine, taskLine } from "./report.ts";
import { profilerLogTake } from "./view.ts";
import { setLongFrameSink } from "./long-frame.ts";

/** How often the stream is collected while it is on. Nothing is pushed: the
 * kernel keeps the rows and this comes for them, in batches rather than per
 * frame — a drain that ran on every frame would be instrumentation paced by
 * the thing it measures. The buffer is capped, so a slow reader loses the
 * oldest rather than growing the kernel's heap. */
const DRAIN_MS = 500;

let drain: ReturnType<typeof setInterval> | null = null;

/** Prints whatever the stream has kept since the last call. */
function drainLog(): void {
  const log = profilerLogTake();
  for (const t of log.tasks) console.log(`[profiler] ${taskLine(t)}`);
  for (const f of log.frames) console.log(`[profiler] ${frameLine(f, null)}`);
}

/**
 * Turns the stream on or off: each settled task and missed frame the
 * kernel keeps, and the shell's long frames as they land. A console's, and
 * so called from one (`__pxl_profiler_log`, overlay.tsx) rather than from a
 * button — where there is no console to read it, as on a released device,
 * there is nothing to turn on. The kernel keeps no stream while the
 * profiler's setting is off, whatever this asks.
 */
export function profilerLog(on: boolean): void {
  kernel_command(Command.PROFILER_LOG, encode_profiler_log({ on }));
  setLongFrameSink(on ? (l) => console.log(`[profiler] ${longFrameLine(l)}`) : null);
  if (on && drain === null) drain = setInterval(drainLog, DRAIN_MS);
  if (!on && drain !== null) {
    clearInterval(drain);
    drain = null;
  }
}
