// @pxl/client/abi/step.spec.ts
//
// GENERATED from spec/client/kernel/turn/step.json
//
// The turn's vocabulary, the kernel's and the shell's: the steps a tick runs, as turn.rs
// registers them, and the effects a turn runs around them. The profiler records and
// reports them by these names (spec/client/kernel/profiler/profiler.json).

export const STEP_NAMES = [
  "controller",
  "input",
  "camera",
  "edit",
  "ghost",
  "environment",
  "companion",
  "preview",
  "demand",
  "scene",
] as const;

export const EFFECT_NAMES = [
  "navigation",
  "bootstrap",
  "orient",
  "follow",
  "dst",
  "decor",
  "objects",
  "puck",
  "settings",
] as const;
