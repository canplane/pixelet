// @xpute/runtime/stop.ts

/** The stop the runtime serves the core (wit/stop.wit): the core's report of
 * a broken invariant, logged and handed to `report`, before the core stops. */

import type { i32, u32, u64 } from "@xpute/kit/abi/word.ts";
import { strerror } from "@xpute/kit/status/errno.spec.ts";
import * as stop from "./abi/stop.ts";
import type { Link } from "./link.ts";

export interface Stop {
  errnum: i32;
  file: string;
  line: u32;
  n1: u64;
  n2: u64;
}

export function serve_stop(link: Pick<Link, "mount">, report: (s: Stop) => void): void {
  stop.mount(link, {
    stopped(errnum, file, line, n1, n2) {
      console.error(`[core] stopped: ${strerror(errnum)} at ${file}:${line} (${n1}, ${n2})`);
      report({ errnum, file, line, n1, n2 });
    },
  });
}
