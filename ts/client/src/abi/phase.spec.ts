// @pxl/client/abi/phase.spec.ts
//
// GENERATED from spec/client/core/sys/sched/phase.json
//
// The core's tick phases, in the order they run (pxl-core sys/sched/tick.rs):
// what moves the eye first, unbudgeted and O(1); then what is visible. A
// phase's number is its place here, and a report of per-phase time is indexed
// by it.

export const PHASES = ["interaction", "visible"] as const;
