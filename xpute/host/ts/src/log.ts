// @xpute/host/log.ts

/** The log the host serves the guest (wit/log/log.wit), and its own: each
 * record, a level, a context and a message, is handed to its runtime
 * (runtime.ts `log`), which writes it, as a native host hands its own
 * (xpute-host log.rs). */

import { mount } from "./abi/log/log.ts";
import type { Level } from "./abi/log/log.enum.ts";
import type { Side } from "@xpute/link";
import { runtime } from "./runtime.ts";

/** Hands its runtime a record of the host's, or of what it serves:
 * `message`, at `level`, about `context`. */
export function log(level: Level, context: string, message: string): void {
  runtime().log(level, context, message);
}

export function serve_log(side: Pick<Side, "mount_named">): void {
  mount(side, { log });
}
