// @pxl/client/abi/demand.spec.ts
//
// GENERATED from spec/client/core/sys/io/demand.json
//
// A request's vocabulary, the core's and the UI's: what stream it is in, how
// its run came to an end, and the phases a run reports its time in. The profiler
// records and reports them by these names (spec/client/core/perf/perf.json).

import type { u32 } from "@xpute/kit/abi/word.ts";

export const STREAM_NAMES = [
  "terrain",
  "place",
  "division",
  "asset",
  "earth",
  "object",
] as const;

export const OUTCOME_NAMES = ["ok", "aborted", "err"] as const;

export const TASK_PHASE_NAMES = [
  "net_ms",
  "bytes",
  "decode_ms",
  "adopt_ms",
] as const;

/** A run's timings past this many are dropped. */
export const TASK_PHASES_MAX: u32 = 6 as number;
