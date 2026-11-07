// @xpute/runtime/sched/wake.ts

import type { f64 } from "@xpute/kit/abi/word.ts";

/** A falling edge's `wake_ms` when the core waits on something only the runtime
 * brings, an input or an arrival: no turn of its own asking. */
export const NO_WAKE: f64 = -1;
