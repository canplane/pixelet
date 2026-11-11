// @xpute/runtime/sched/wake.ts

import type { f64 } from "@xpute/kit/abi/word.ts";

/** A turn answers this `wake_ms` when the core asks for no turn of its
 * own, because it waits on something only the runtime brings, such as an input
 * or an arrival. */
export const NO_WAKE: f64 = -1;
