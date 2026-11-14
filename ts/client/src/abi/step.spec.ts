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

export type StepName =
  | typeof STEP_NAME_CONTROLLER
  | typeof STEP_NAME_INPUT
  | typeof STEP_NAME_CAMERA
  | typeof STEP_NAME_EDIT
  | typeof STEP_NAME_GHOST
  | typeof STEP_NAME_ENVIRONMENT
  | typeof STEP_NAME_COMPANION
  | typeof STEP_NAME_DEMAND
  | typeof STEP_NAME_SCENE;
export const STEP_NAME_CONTROLLER = 0;
export const STEP_NAME_INPUT = 1;
export const STEP_NAME_CAMERA = 2;
export const STEP_NAME_EDIT = 3;
export const STEP_NAME_GHOST = 4;
export const STEP_NAME_ENVIRONMENT = 5;
export const STEP_NAME_COMPANION = 6;
export const STEP_NAME_DEMAND = 7;
export const STEP_NAME_SCENE = 8;

/** The member `val` is, or null for a number no member has. */
export function of_StepName(val: number): StepName | null {
  return val === STEP_NAME_CONTROLLER ||
    val === STEP_NAME_INPUT ||
    val === STEP_NAME_CAMERA ||
    val === STEP_NAME_EDIT ||
    val === STEP_NAME_GHOST ||
    val === STEP_NAME_ENVIRONMENT ||
    val === STEP_NAME_COMPANION ||
    val === STEP_NAME_DEMAND ||
    val === STEP_NAME_SCENE
    ? val
    : null;
}

export const EFFECT_NAMES = [
  "navigation",
  "bootstrap",
  "orient",
  "follow",
  "dst",
  "decor",
  "object",
  "location",
  "setting",
] as const;

export type EffectName =
  | typeof EFFECT_NAME_NAVIGATION
  | typeof EFFECT_NAME_BOOTSTRAP
  | typeof EFFECT_NAME_ORIENT
  | typeof EFFECT_NAME_FOLLOW
  | typeof EFFECT_NAME_DST
  | typeof EFFECT_NAME_DECOR
  | typeof EFFECT_NAME_OBJECT
  | typeof EFFECT_NAME_LOCATION
  | typeof EFFECT_NAME_SETTING;
export const EFFECT_NAME_NAVIGATION = 0;
export const EFFECT_NAME_BOOTSTRAP = 1;
export const EFFECT_NAME_ORIENT = 2;
export const EFFECT_NAME_FOLLOW = 3;
export const EFFECT_NAME_DST = 4;
export const EFFECT_NAME_DECOR = 5;
export const EFFECT_NAME_OBJECT = 6;
export const EFFECT_NAME_LOCATION = 7;
export const EFFECT_NAME_SETTING = 8;

/** The member `val` is, or null for a number no member has. */
export function of_EffectName(val: number): EffectName | null {
  return val === EFFECT_NAME_NAVIGATION ||
    val === EFFECT_NAME_BOOTSTRAP ||
    val === EFFECT_NAME_ORIENT ||
    val === EFFECT_NAME_FOLLOW ||
    val === EFFECT_NAME_DST ||
    val === EFFECT_NAME_DECOR ||
    val === EFFECT_NAME_OBJECT ||
    val === EFFECT_NAME_LOCATION ||
    val === EFFECT_NAME_SETTING
    ? val
    : null;
}
