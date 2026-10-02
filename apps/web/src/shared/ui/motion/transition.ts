// @pxl/web/shared/ui/motion/transition.ts

/**
 * Every move is solved here into a sampled curve, which each platform plays
 * on its compositor. Nothing runs per frame: the kernel spends the JS
 * thread's frame, so a per-frame curve would stutter while loading.
 */

import type { f64 } from "@xpute/core/abi/word.ts";

/** Samples from 0 to 1 at evenly spaced moments (past 1 where a spring overshoots). */
export interface Transition {
  durationMs: f64;
  samples: readonly f64[];
  delayMs?: f64;
}

/** Settled once it stays this close to the target, as a share of the move. */
const REST = 0.001;
const STEP_MS = 1;
const MAX_MS = 3000;
const SAMPLES = 64;

/** A spring's constants: m x'' + c x' + k x = k, from rest. */
interface SpringConstants {
  stiffness: f64;
  damping: f64;
  mass: f64;
}

function position(s: SpringConstants, seconds: f64): f64 {
  const w0 = Math.sqrt(s.stiffness / s.mass);
  const zeta = s.damping / (2 * Math.sqrt(s.stiffness * s.mass));
  if (zeta < 1) {
    const wd = w0 * Math.sqrt(1 - zeta * zeta);
    return 1 - Math.exp(-zeta * w0 * seconds) * (Math.cos(wd * seconds) + (zeta * w0 / wd) * Math.sin(wd * seconds));
  }
  if (zeta === 1) return 1 - Math.exp(-w0 * seconds) * (1 + w0 * seconds);
  const root = Math.sqrt(zeta * zeta - 1);
  const r1 = -w0 * (zeta - root);
  const r2 = -w0 * (zeta + root);
  return 1 - (r2 * Math.exp(r1 * seconds) - r1 * Math.exp(r2 * seconds)) / (r2 - r1);
}

export function spring(s: SpringConstants): Transition {
  let lastMs = 0;
  for (let t = 0; t <= MAX_MS; t += STEP_MS) {
    if (Math.abs(1 - position(s, t / 1000)) > REST) lastMs = t;
  }
  const durationMs = lastMs + STEP_MS;
  const samples = Array.from({ length: SAMPLES + 1 }, (_, i) => (i === SAMPLES ? 1 : position(s, (durationMs * i) / SAMPLES / 1000)));
  return { durationMs, samples };
}

/** Bisection is safe: x rises monotonically for every control point CSS allows. */
function bezierAt([x1, y1, x2, y2]: readonly [f64, f64, f64, f64], x: f64): f64 {
  const at = (a: f64, b: f64, t: f64) => 3 * a * t * (1 - t) ** 2 + 3 * b * t * t * (1 - t) + t ** 3;
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 32; i++) {
    const mid = (lo + hi) / 2;
    if (at(x1, x2, mid) < x) lo = mid;
    else hi = mid;
  }
  return at(y1, y2, (lo + hi) / 2);
}

export function ease(durationMs: f64, bezier: readonly [f64, f64, f64, f64]): Transition {
  return { durationMs, samples: Array.from({ length: SAMPLES + 1 }, (_, i) => bezierAt(bezier, i / SAMPLES)) };
}

export function curveAt(t: Transition, p: f64): f64 {
  const at = Math.min(1, Math.max(0, p)) * (t.samples.length - 1);
  const i = Math.floor(at);
  if (i >= t.samples.length - 1) return t.samples[t.samples.length - 1];
  return t.samples[i] + (t.samples[i + 1] - t.samples[i]) * (at - i);
}

/** Overdamped, so a target that moves mid-flight does not ring. */
export const SNAP: Transition = spring({ stiffness: 500, damping: 40, mass: 0.5 });

export const SETTLE: Transition = ease(300, [0, 0, 0.58, 1]);

export const LEAVE: Transition = ease(150, [0, 0, 0.58, 1]);

export const FADE: Transition = ease(200, [0, 0, 0.58, 1]);

export const ARRIVE: Transition = ease(600, [0.16, 1, 0.3, 1]);

/** Critical damping: leaves at full speed and lands without a bounce. An
 * ease-in-out moved a panel 8% in 100 ms and read as slow. */
export const SLIDE: Transition = spring({ stiffness: 420, damping: 41, mass: 1 });

/** Material's emphasized accelerate; a spring would linger at the end. */
export const SLIDE_AWAY: Transition = ease(200, [0.3, 0, 0.8, 0.15]);

export const DIM: Transition = ease(500, [0.42, 0, 0.58, 1]);

export function after(t: Transition, delayMs: f64): Transition {
  return { ...t, delayMs };
}
