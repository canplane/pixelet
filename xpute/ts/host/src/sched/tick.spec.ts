// @xpute/host/sched/tick.spec.ts
//
// GENERATED from spec/sched/tick.json — do not edit.
//
// A tick's phases, in the order they run and are prioritized: interaction unbudgeted,
// then what is visible, then content, then the cosmetic and the report. A phase's number
// is its place here, and a report of per-phase time is indexed by it.

export const PHASES = ["interaction", "visible", "content", "cosmetic", "report"] as const;
