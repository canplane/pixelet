// @pxl/web/shared/ui/motion/transition.ts

/**
 * How a pose is reached, said once for both platforms. A spring is solved
 * here into a curve over a fixed duration, and each platform plays that curve
 * on its compositor: the page as a CSS `linear()` easing, a device as the
 * easing of a native-driven timing. So a spring moves the same on both, and
 * settles at the same moment, which is what an exit waits on.
 *
 * Nothing here runs per frame. The kernel spends the JavaScript thread's
 * frame (xpute-runtime sched/quantum.rs), so a curve evaluated there would
 * stutter whenever the world is loading; a curve handed over whole does not.
 */

import type { f64 } from "@xpute/core/abi/word.ts";

export interface Spring {
  type: "spring";
  stiffness: f64;
  damping: f64;
  mass: f64;
  delayMs?: f64;
}

export interface Ease {
  type: "ease";
  durationMs: f64;
  /** A cubic Bézier's two control points, as CSS's `cubic-bezier()`. */
  bezier: readonly [f64, f64, f64, f64];
  delayMs?: f64;
}

export type Transition = Spring | Ease;

/** A spring solved: how long it runs, and where it stands at evenly spaced
 * moments across that time, from 0 to 1 (past 1 where it overshoots). */
export interface SpringCurve {
  durationMs: f64;
  samples: readonly f64[];
}

/** Settled once it stays this close to the target, as a share of the move. */
const REST = 0.001;
const STEP_MS = 1;
const MAX_MS = 3000;
const SAMPLES = 64;

/** By the spring's constants, not its object: a delay makes a new object of
 * the same spring. */
const solved = new Map<string, SpringCurve>();

/** A unit step response of a damped spring from rest: m x'' + c x' + k x = k. */
function position(s: Spring, seconds: f64): f64 {
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

export function springCurve(s: Spring): SpringCurve {
  const key = `${s.stiffness}/${s.damping}/${s.mass}`;
  const hit = solved.get(key);
  if (hit) return hit;
  let lastMs = 0;
  for (let t = 0; t <= MAX_MS; t += STEP_MS) {
    if (Math.abs(1 - position(s, t / 1000)) > REST) lastMs = t;
  }
  const durationMs = lastMs + STEP_MS;
  const samples = Array.from({ length: SAMPLES + 1 }, (_, i) => (i === SAMPLES ? 1 : position(s, (durationMs * i) / SAMPLES / 1000)));
  const curve = { durationMs, samples };
  solved.set(key, curve);
  return curve;
}

/** How long a transition runs, its delay not counted. */
export function durationMs(t: Transition): f64 {
  return t.type === "spring" ? springCurve(t).durationMs : t.durationMs;
}

/** Quick and settled, for what moves under the finger's eye: a control's
 * shape and place. Over critical damping, so a target that moves again
 * mid-flight does not ring. */
export const SNAP: Spring = { type: "spring", stiffness: 500, damping: 40, mass: 0.5 };

/** A card of the deck finding its depth. */
export const SETTLE: Ease = { type: "ease", durationMs: 300, bezier: [0, 0, 0.58, 1] };

/** Something leaving: quicker than it came. */
export const LEAVE: Ease = { type: "ease", durationMs: 150, bezier: [0, 0, 0.58, 1] };

/** A control arriving on the map: a long, soft landing. */
export const ARRIVE: Ease = { type: "ease", durationMs: 600, bezier: [0.16, 1, 0.3, 1] };

/** A panel sliding over the map, and the chrome dimming while the camera is
 * busy: even at both ends. */
export const SLIDE: Ease = { type: "ease", durationMs: 500, bezier: [0.42, 0, 0.58, 1] };

/** A panel sliding away: the same curve, sooner done. */
export const SLIDE_AWAY: Ease = { ...SLIDE, durationMs: 300 };

/** A delay added to a transition. */
export function after<T extends Transition>(t: T, delayMs: f64): T {
  return { ...t, delayMs };
}

/** A spring's curve at progress `p` from 0 to 1, straight between its
 * samples, as CSS's `linear()` reads them. */
export function curveAt(c: SpringCurve, p: f64): f64 {
  const at = Math.min(1, Math.max(0, p)) * (c.samples.length - 1);
  const i = Math.floor(at);
  if (i >= c.samples.length - 1) return c.samples[c.samples.length - 1];
  return c.samples[i] + (c.samples[i + 1] - c.samples[i]) * (at - i);
}
