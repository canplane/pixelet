// @pxl/client/abi/phase.spec.ts
//
// GENERATED from spec/client/core/sys/sched/phase.json (xpute-gen data.rs)
//
// The core's tick phases, in the order they run (pxl-core sys/sched/tick.rs):
// what moves the eye first, unbudgeted and O(1); then what is visible. A
// phase's number is its place here, and a report of per-phase time is indexed
// by it.

export const PHASES = ["interaction", "visible"] as const;

export const Phase = {
  interaction: 0,
  visible: 1,
} as const;
export type Phase = (typeof Phase)[keyof typeof Phase];

/** The name of the `Phase` member numbered `n`, or undefined for a number
 * no member has. */
export function phase_name(n: number): keyof typeof Phase | undefined {
  return (Object.keys(Phase) as (keyof typeof Phase)[]).find(
    (k) => Phase[k] === n,
  );
}

/** The `Phase` member `n` is, or null for a number no member has. */
export function phase_of(n: number): Phase | null {
  return phase_name(n) === undefined ? null : (n as Phase);
}
