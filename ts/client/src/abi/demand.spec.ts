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

export const StreamName = {
  terrain: 0,
  place: 1,
  division: 2,
  asset: 3,
  earth: 4,
  object: 5,
} as const;
export type StreamName = (typeof StreamName)[keyof typeof StreamName];

/** The name of the `StreamName` member numbered `n`, or undefined for a number
 * no member has. */
export function stream_name_name(
  n: number,
): keyof typeof StreamName | undefined {
  for (const k in StreamName)
    if (StreamName[k as keyof typeof StreamName] === n)
      return k as keyof typeof StreamName;
  return undefined;
}

/** The `StreamName` member `n` is, or null for a number no member has. */
export function stream_name_of(n: number): StreamName | null {
  return stream_name_name(n) === undefined ? null : (n as StreamName);
}

export const OUTCOME_NAMES = ["ok", "aborted", "err"] as const;

export const OutcomeName = {
  ok: 0,
  aborted: 1,
  err: 2,
} as const;
export type OutcomeName = (typeof OutcomeName)[keyof typeof OutcomeName];

/** The name of the `OutcomeName` member numbered `n`, or undefined for a number
 * no member has. */
export function outcome_name_name(
  n: number,
): keyof typeof OutcomeName | undefined {
  for (const k in OutcomeName)
    if (OutcomeName[k as keyof typeof OutcomeName] === n)
      return k as keyof typeof OutcomeName;
  return undefined;
}

/** The `OutcomeName` member `n` is, or null for a number no member has. */
export function outcome_name_of(n: number): OutcomeName | null {
  return outcome_name_name(n) === undefined ? null : (n as OutcomeName);
}

export const TASK_PHASE_NAMES = ["net_ms", "bytes", "decode_ms"] as const;

export const TaskPhaseName = {
  net_ms: 0,
  bytes: 1,
  decode_ms: 2,
} as const;
export type TaskPhaseName = (typeof TaskPhaseName)[keyof typeof TaskPhaseName];

/** The name of the `TaskPhaseName` member numbered `n`, or undefined for a number
 * no member has. */
export function task_phase_name_name(
  n: number,
): keyof typeof TaskPhaseName | undefined {
  for (const k in TaskPhaseName)
    if (TaskPhaseName[k as keyof typeof TaskPhaseName] === n)
      return k as keyof typeof TaskPhaseName;
  return undefined;
}

/** The `TaskPhaseName` member `n` is, or null for a number no member has. */
export function task_phase_name_of(n: number): TaskPhaseName | null {
  return task_phase_name_name(n) === undefined ? null : (n as TaskPhaseName);
}

/** A run's timings past this many are dropped. */
export const TASK_PHASES_MAX = 6 as number;
