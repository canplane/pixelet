// @xpute/host/runtime.ts

/** What the host needs of the runtime it runs in, which the runtime
 * implements and gives it once, before it mounts any implementation (`init`),
 * as the native runtime gives xpute-host its own (runtime.rs `Runtime`):
 * where the guest's stop is reported, and where a log record is written. */

import { bug } from "@xpute/kit/status/bug.ts";
import {
  ERRNO_EALREADY,
  ERRNO_ENOTRECOVERABLE,
} from "@xpute/kit/status/errno.spec.ts";
import type { i32, u32, u64 } from "@xpute/kit/abi/word.ts";
import type { Level } from "./abi/log/log.enum.ts";

/** The report of a broken invariant before the guest stops: the guest's
 * (wit/stop/stop.wit), which the host that implements the stop hands its
 * runtime (stop.ts), or the host's own failure, which its runtime reports
 * itself, as a native host's is (xpute-host runtime.rs `Stop`). Its message is
 * a panic's or a throw's own, or the bug's where it has one. */
export interface Stop {
  errnum: i32;
  file: string;
  line: u32;
  n1: u64;
  n2: u64;
  msg: string | null;
}

/** A stop as the UI hears it: the report without its message, which the
 * runtime alone writes, as the native listener carries it. It sits here with
 * `Stop`, whose shape it takes, and the UI end takes it from the host. */
export type UiStop = Omit<Stop, "msg">;

export interface Runtime {
  /** Reports the guest's stop. */
  stopped(stop: Stop): void;
  /** Writes a log record: its level, its context and its message. */
  log(level: Level, context: string, msg: string): void;
}

let cur: Runtime | null = null;

/** Gives the host its runtime, once. */
export function init(runtime: Runtime): void {
  if (cur !== null) bug(ERRNO_EALREADY);
  cur = runtime;
}

/** The runtime `init` gave, which nothing reads before it. */
export function runtime(): Runtime {
  return cur ?? bug(ERRNO_ENOTRECOVERABLE);
}
