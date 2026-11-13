// @xpute/link/stop.ts

/** A guest's report of a broken invariant as it stops (wit/stop/stop.wit),
 * which a host serves and hands on. */

import type { i32, u32, u64 } from "@xpute/kit/abi/word.ts";

export interface Stop {
  errnum: i32;
  file: string;
  line: u32;
  n1: u64;
  n2: u64;
}
