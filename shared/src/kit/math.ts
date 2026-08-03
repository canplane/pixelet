// @pixelet/shared/kit/math.ts

import type { f64 } from "@pixelet/shared/kit/word.ts";

export const EPS = 1e-5 as const;

/**
 * Precision equality check using both absolute and relative tolerance.
 * As magnitudes grow, the allowed error scales proportionally with them.
 */
export const approx_eq = (a: f64, b: f64, eps: f64 = EPS): boolean => {
  if (a === b) return true;
  const diff = Math.abs(a - b);
  // absolute tolerance (small values) || relative tolerance (large values)
  return diff < eps || diff < eps * Math.max(Math.abs(a), Math.abs(b));
};

/** Physical/intuitive proximity check using only absolute distance. */
export const near = (a: f64, b: f64, tol: f64): boolean => Math.abs(a - b) < tol;

export const lerp = (a: f64, b: f64, t: f64): f64 => a + (b - a) * t;
export const inv_lerp = (x: f64, min: f64, max: f64): f64 => (x - min) / (max - min);

/** Remaps a value from one range into another. */
export const remap = (x: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64): f64 => lerp(out_min, out_max, inv_lerp(x, in_min, in_max));

export const clamp = (x: f64, min: f64, max: f64): f64 => x < min ? min : x > max ? max : x;
export const wrap = (x: f64, min: f64, max: f64): f64 => {
  const range = max - min;
  return ((((x - min) % range) + range) % range) + min;
};

export const floor_to_step = (x: f64, step: f64): f64 => Math.floor(x / step) * step;
export const round_to_step = (x: f64, step: f64): f64 => Math.round(x / step) * step;
export const ceil_to_step = (x: f64, step: f64): f64 => Math.ceil(x / step) * step;
