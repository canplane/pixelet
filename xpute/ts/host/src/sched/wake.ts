// @xpute/host/sched/wake.ts

import type { f64 } from "@xpute/kit/abi/word.ts";

/** A falling edge's `wake_ms` when the guest waits on something only the host
 * brings, an input or an arrival: no turn of its own asking. */
export const NO_WAKE: f64 = -1;
