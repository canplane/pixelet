// @pxl/client/abi/phase.spec.ts
//
// GENERATED from spec/client/core/sys/sched/phase.json5 (xpute-gen data.rs)
//
// The core's tick phases, in the order they run (pxl-core sys/sched/tick.rs):
// what moves the eye first, unbudgeted and O(1); then what is visible. A
// phase's number is its place here, and a report of per-phase time is indexed
// by it.

export const PHASES = ["interaction", "visible"] as const;

export type Phase = typeof PHASE_INTERACTION | typeof PHASE_VISIBLE;
export const PHASE_INTERACTION = 0;
export const PHASE_VISIBLE = 1;

/** The member `val` is, or null for a number no member has. */
export function of_Phase(val: number): Phase | null {
  return val === PHASE_INTERACTION || val === PHASE_VISIBLE ? val : null;
}
