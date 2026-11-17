// @pxl/client/ui/shared/component/motion/transition.ts

/**
 * The curves the UI's motions take, each a duration and a cubic Bézier, as
 * CSS writes a timing function. react-native-reanimated plays them off the JS
 * thread, on the web and natively alike.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";

export interface Transition {
  durationMs: f64;
  bezier: readonly [f64, f64, f64, f64];
  delayMs?: f64;
}

/** Leaves fast and lands without a bounce, so that a target moving mid-flight
 * does not make the view ring. */
export const SNAP: Transition = {
  durationMs: 464,
  bezier: [0.24, 0.8, 0.12, 1],
};

export const SETTLE: Transition = { durationMs: 300, bezier: [0, 0, 0.58, 1] };

export const LEAVE: Transition = { durationMs: 150, bezier: [0, 0, 0.58, 1] };

export const FADE: Transition = { durationMs: 200, bezier: [0, 0, 0.58, 1] };

/** A sheet's slide in: it leaves at full speed and lands without a bounce. */
export const SLIDE: Transition = {
  durationMs: 451,
  bezier: [0.32, 0.68, 0.12, 1],
};

/** Material Design's emphasized accelerate easing. */
export const SLIDE_AWAY: Transition = {
  durationMs: 200,
  bezier: [0.3, 0, 0.8, 0.15],
};

export const DIM: Transition = { durationMs: 500, bezier: [0.42, 0, 0.58, 1] };

/** Overshoots its end and comes back to it. */
export const POP: Transition = {
  durationMs: 400,
  bezier: [0.34, 1.56, 0.64, 1],
};

export function after(t: Transition, delayMs: f64): Transition {
  return { ...t, delayMs };
}
