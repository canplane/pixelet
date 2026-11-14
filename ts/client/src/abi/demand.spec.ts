// @pxl/client/abi/demand.spec.ts
//
// GENERATED from spec/client/core/sys/io/demand.json (xpute-gen data.rs)
//
// A request's vocabulary, the core's and the UI's: what stream it is in, how
// its run came to an end, and the phases a run reports its time in. The profiler
// records and reports them by these names (spec/client/core/perf/perf.json).

export const STREAM_NAMES = [
  "terrain",
  "place",
  "division",
  "asset",
  "earth",
  "object",
] as const;

export type StreamName =
  | typeof STREAM_NAME_TERRAIN
  | typeof STREAM_NAME_PLACE
  | typeof STREAM_NAME_DIVISION
  | typeof STREAM_NAME_ASSET
  | typeof STREAM_NAME_EARTH
  | typeof STREAM_NAME_OBJECT;
export const STREAM_NAME_TERRAIN = 0;
export const STREAM_NAME_PLACE = 1;
export const STREAM_NAME_DIVISION = 2;
export const STREAM_NAME_ASSET = 3;
export const STREAM_NAME_EARTH = 4;
export const STREAM_NAME_OBJECT = 5;

/** The member `val` is, or null for a number no member has. */
export function of_StreamName(val: number): StreamName | null {
  return val === STREAM_NAME_TERRAIN ||
    val === STREAM_NAME_PLACE ||
    val === STREAM_NAME_DIVISION ||
    val === STREAM_NAME_ASSET ||
    val === STREAM_NAME_EARTH ||
    val === STREAM_NAME_OBJECT
    ? val
    : null;
}

export const OUTCOME_NAMES = ["ok", "aborted", "err"] as const;

export type OutcomeName =
  | typeof OUTCOME_NAME_OK
  | typeof OUTCOME_NAME_ABORTED
  | typeof OUTCOME_NAME_ERR;
export const OUTCOME_NAME_OK = 0;
export const OUTCOME_NAME_ABORTED = 1;
export const OUTCOME_NAME_ERR = 2;

/** The member `val` is, or null for a number no member has. */
export function of_OutcomeName(val: number): OutcomeName | null {
  return val === OUTCOME_NAME_OK ||
    val === OUTCOME_NAME_ABORTED ||
    val === OUTCOME_NAME_ERR
    ? val
    : null;
}

export const TASK_PHASE_NAMES = ["net_ms", "bytes", "decode_ms"] as const;

export type TaskPhaseName =
  | typeof TASK_PHASE_NAME_NET_MS
  | typeof TASK_PHASE_NAME_BYTES
  | typeof TASK_PHASE_NAME_DECODE_MS;
export const TASK_PHASE_NAME_NET_MS = 0;
export const TASK_PHASE_NAME_BYTES = 1;
export const TASK_PHASE_NAME_DECODE_MS = 2;

/** The member `val` is, or null for a number no member has. */
export function of_TaskPhaseName(val: number): TaskPhaseName | null {
  return val === TASK_PHASE_NAME_NET_MS ||
    val === TASK_PHASE_NAME_BYTES ||
    val === TASK_PHASE_NAME_DECODE_MS
    ? val
    : null;
}

/** A run's timings past this many are dropped. */
export const TASK_PHASE_CAP = 6 as number;
