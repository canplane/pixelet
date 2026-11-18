// @pxl/client/abi/step.spec.ts
//
// GENERATED from spec/client/core/perf/step.json5 (xpute-gen data.rs)
//
// The frame's vocabulary, the core's and the UI's: the steps a tick runs, as frame.rs
// registers them, and the phase each runs in, the effects a frame runs before them, the parts of the stage it runs after them and the requests' pump, and the reasons a frame is asked for. The profiler records and
// reports them by these names (spec/client/core/perf/perf.json5).

import type { Phase } from "./phase.spec.ts";

export const STEP_NAMES = [
  "controller",
  "input",
  "camera",
  "edit",
  "ghost",
  "location",
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
  | typeof STEP_NAME_LOCATION
  | typeof STEP_NAME_ENVIRONMENT
  | typeof STEP_NAME_COMPANION
  | typeof STEP_NAME_DEMAND
  | typeof STEP_NAME_SCENE;
export const STEP_NAME_CONTROLLER = 0;
export const STEP_NAME_INPUT = 1;
export const STEP_NAME_CAMERA = 2;
export const STEP_NAME_EDIT = 3;
export const STEP_NAME_GHOST = 4;
export const STEP_NAME_LOCATION = 5;
export const STEP_NAME_ENVIRONMENT = 6;
export const STEP_NAME_COMPANION = 7;
export const STEP_NAME_DEMAND = 8;
export const STEP_NAME_SCENE = 9;

/** The member `val` is, or null for a number no member has. */
export function of_StepName(val: number): StepName | null {
  return val === STEP_NAME_CONTROLLER ||
    val === STEP_NAME_INPUT ||
    val === STEP_NAME_CAMERA ||
    val === STEP_NAME_EDIT ||
    val === STEP_NAME_GHOST ||
    val === STEP_NAME_LOCATION ||
    val === STEP_NAME_ENVIRONMENT ||
    val === STEP_NAME_COMPANION ||
    val === STEP_NAME_DEMAND ||
    val === STEP_NAME_SCENE
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_StepName(val: StepName): string {
  switch (val) {
    case STEP_NAME_CONTROLLER:
      return "controller";
    case STEP_NAME_INPUT:
      return "input";
    case STEP_NAME_CAMERA:
      return "camera";
    case STEP_NAME_EDIT:
      return "edit";
    case STEP_NAME_GHOST:
      return "ghost";
    case STEP_NAME_LOCATION:
      return "location";
    case STEP_NAME_ENVIRONMENT:
      return "environment";
    case STEP_NAME_COMPANION:
      return "companion";
    case STEP_NAME_DEMAND:
      return "demand";
    case STEP_NAME_SCENE:
      return "scene";
  }
}

/** The phase each step runs in, in STEP_NAME's order (pxl-core frame.rs `tick`). */
export const STEP_PHASE = [
  0 as Phase,
  0 as Phase,
  0 as Phase,
  0 as Phase,
  0 as Phase,
  0 as Phase,
  0 as Phase,
  0 as Phase,
  1 as Phase,
  1 as Phase,
] as const;

export const EFFECT_NAMES = [
  "navigation",
  "bootstrap",
  "follow",
  "dst",
  "decor",
  "object",
  "setting",
] as const;

export type EffectName =
  | typeof EFFECT_NAME_NAVIGATION
  | typeof EFFECT_NAME_BOOTSTRAP
  | typeof EFFECT_NAME_FOLLOW
  | typeof EFFECT_NAME_DST
  | typeof EFFECT_NAME_DECOR
  | typeof EFFECT_NAME_OBJECT
  | typeof EFFECT_NAME_SETTING;
export const EFFECT_NAME_NAVIGATION = 0;
export const EFFECT_NAME_BOOTSTRAP = 1;
export const EFFECT_NAME_FOLLOW = 2;
export const EFFECT_NAME_DST = 3;
export const EFFECT_NAME_DECOR = 4;
export const EFFECT_NAME_OBJECT = 5;
export const EFFECT_NAME_SETTING = 6;

/** The member `val` is, or null for a number no member has. */
export function of_EffectName(val: number): EffectName | null {
  return val === EFFECT_NAME_NAVIGATION ||
    val === EFFECT_NAME_BOOTSTRAP ||
    val === EFFECT_NAME_FOLLOW ||
    val === EFFECT_NAME_DST ||
    val === EFFECT_NAME_DECOR ||
    val === EFFECT_NAME_OBJECT ||
    val === EFFECT_NAME_SETTING
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_EffectName(val: EffectName): string {
  switch (val) {
    case EFFECT_NAME_NAVIGATION:
      return "navigation";
    case EFFECT_NAME_BOOTSTRAP:
      return "bootstrap";
    case EFFECT_NAME_FOLLOW:
      return "follow";
    case EFFECT_NAME_DST:
      return "dst";
    case EFFECT_NAME_DECOR:
      return "decor";
    case EFFECT_NAME_OBJECT:
      return "object";
    case EFFECT_NAME_SETTING:
      return "setting";
  }
}

/** The parts of a frame's stage, in the order they start (pxl-core domain/scene/stage.rs `stage_render`): `assemble` what the renderer's frame is made from, the eye's cast among it; `upload` the ground's bricks and the imagery's tiles handed to the renderer, which asks for them as it encodes; `encode` the renderer's frame, less those uploads; `judge` what the frame leaves, the images it took given back and the drawn resolution judged. */
export const STAGE_PART_NAMES = [
  "assemble",
  "upload",
  "encode",
  "judge",
] as const;

export type StagePartName =
  | typeof STAGE_PART_NAME_ASSEMBLE
  | typeof STAGE_PART_NAME_UPLOAD
  | typeof STAGE_PART_NAME_ENCODE
  | typeof STAGE_PART_NAME_JUDGE;
export const STAGE_PART_NAME_ASSEMBLE = 0;
export const STAGE_PART_NAME_UPLOAD = 1;
export const STAGE_PART_NAME_ENCODE = 2;
export const STAGE_PART_NAME_JUDGE = 3;

/** The member `val` is, or null for a number no member has. */
export function of_StagePartName(val: number): StagePartName | null {
  return val === STAGE_PART_NAME_ASSEMBLE ||
    val === STAGE_PART_NAME_UPLOAD ||
    val === STAGE_PART_NAME_ENCODE ||
    val === STAGE_PART_NAME_JUDGE
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_StagePartName(val: StagePartName): string {
  switch (val) {
    case STAGE_PART_NAME_ASSEMBLE:
      return "assemble";
    case STAGE_PART_NAME_UPLOAD:
      return "upload";
    case STAGE_PART_NAME_ENCODE:
      return "encode";
    case STAGE_PART_NAME_JUDGE:
      return "judge";
  }
}

/** Why a frame is asked for (pxl-core sys/sched.rs `invalidate`): `method` a method from outside applied (the UI's, a surface's among them), `view` the camera moved or the ratio the world is drawn at changed, `scene` what is shown changed, `completion` something outside the frame finished and a frame takes it (a read, a parse, a write, the GPU's), `pending` a frame stopped short of its work or put it off and a later one goes on, `animation` an animation's step or a frame of its motion. */
export const INVALIDATION_REASONS = [
  "method",
  "view",
  "scene",
  "completion",
  "pending",
  "animation",
] as const;

export type InvalidationReason =
  | typeof INVALIDATION_REASON_METHOD
  | typeof INVALIDATION_REASON_VIEW
  | typeof INVALIDATION_REASON_SCENE
  | typeof INVALIDATION_REASON_COMPLETION
  | typeof INVALIDATION_REASON_PENDING
  | typeof INVALIDATION_REASON_ANIMATION;
export const INVALIDATION_REASON_METHOD = 0;
export const INVALIDATION_REASON_VIEW = 1;
export const INVALIDATION_REASON_SCENE = 2;
export const INVALIDATION_REASON_COMPLETION = 3;
export const INVALIDATION_REASON_PENDING = 4;
export const INVALIDATION_REASON_ANIMATION = 5;

/** The member `val` is, or null for a number no member has. */
export function of_InvalidationReason(val: number): InvalidationReason | null {
  return val === INVALIDATION_REASON_METHOD ||
    val === INVALIDATION_REASON_VIEW ||
    val === INVALIDATION_REASON_SCENE ||
    val === INVALIDATION_REASON_COMPLETION ||
    val === INVALIDATION_REASON_PENDING ||
    val === INVALIDATION_REASON_ANIMATION
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_InvalidationReason(val: InvalidationReason): string {
  switch (val) {
    case INVALIDATION_REASON_METHOD:
      return "method";
    case INVALIDATION_REASON_VIEW:
      return "view";
    case INVALIDATION_REASON_SCENE:
      return "scene";
    case INVALIDATION_REASON_COMPLETION:
      return "completion";
    case INVALIDATION_REASON_PENDING:
      return "pending";
    case INVALIDATION_REASON_ANIMATION:
      return "animation";
  }
}
