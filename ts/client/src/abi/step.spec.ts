// @pxl/client/abi/step.spec.ts
//
// GENERATED from spec/client/core/perf/step.json (xpute-gen data.rs)
//
// The frame's vocabulary, the core's and the UI's: the steps a tick runs, as frame.rs
// registers them, and the effects a frame runs around them. The profiler records and
// reports them by these names (spec/client/core/perf/perf.json).

export const STEP_NAMES = [
  "controller",
  "input",
  "camera",
  "edit",
  "ghost",
  "environment",
  "companion",
  "demand",
  "scene",
] as const;

export const StepName = {
  controller: 0,
  input: 1,
  camera: 2,
  edit: 3,
  ghost: 4,
  environment: 5,
  companion: 6,
  demand: 7,
  scene: 8,
} as const;
export type StepName = (typeof StepName)[keyof typeof StepName];

/** The name of the `StepName` member numbered `n`, or undefined for a number
 * no member has. */
export function step_name_name(n: number): keyof typeof StepName | undefined {
  return (Object.keys(StepName) as (keyof typeof StepName)[]).find(
    (k) => StepName[k] === n,
  );
}

/** The `StepName` member `n` is, or null for a number no member has. */
export function step_name_of(n: number): StepName | null {
  return step_name_name(n) === undefined ? null : (n as StepName);
}

export const EFFECT_NAMES = [
  "navigation",
  "bootstrap",
  "orient",
  "follow",
  "dst",
  "decor",
  "objects",
  "location",
  "settings",
] as const;

export const EffectName = {
  navigation: 0,
  bootstrap: 1,
  orient: 2,
  follow: 3,
  dst: 4,
  decor: 5,
  objects: 6,
  location: 7,
  settings: 8,
} as const;
export type EffectName = (typeof EffectName)[keyof typeof EffectName];

/** The name of the `EffectName` member numbered `n`, or undefined for a number
 * no member has. */
export function effect_name_name(
  n: number,
): keyof typeof EffectName | undefined {
  return (Object.keys(EffectName) as (keyof typeof EffectName)[]).find(
    (k) => EffectName[k] === n,
  );
}

/** The `EffectName` member `n` is, or null for a number no member has. */
export function effect_name_of(n: number): EffectName | null {
  return effect_name_name(n) === undefined ? null : (n as EffectName);
}
