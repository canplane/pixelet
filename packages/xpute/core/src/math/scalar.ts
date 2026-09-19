// @xpute/core/math/scalar.ts

import type { f64 } from "@xpute/core/abi/word.ts";

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

/**
 * The three ways a half rounds, each by the name of where it goes. The
 * project rounds to even — IEEE 754's default, and what WGSL's `round`
 * does — so a value the CPU and a shader both round lands on one integer;
 * the other two are here for the places that have to agree with a
 * language that chose otherwise.
 */

/** Toward +∞: `Math.round`, Java's. `-2.5` is `-2`. */
export const round_half_up = (x: f64): f64 => Math.round(x);

/** Away from zero: C's `round`, Rust's. `-2.5` is `-3`. */
export const round_half_away = (x: f64): f64 => Math.sign(x) * Math.round(Math.abs(x));

/** To even: IEEE 754's, WGSL's, Python's. `2.5` is `2`, `-2.5` is `-2`. */
export const round_half_even = (x: f64): f64 => {
  const r = Math.round(x);
  return Math.abs(x - Math.trunc(x)) === 0.5 && r % 2 !== 0 ? r - 1 : r;
};

/** The project's rounding, which is to even. One line to change if it ever
 * is not; `Math.round` is not it and does not appear in the project. */
export const round = round_half_even;

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
export const round_to_step = (x: f64, step: f64): f64 => round(x / step) * step;
export const ceil_to_step = (x: f64, step: f64): f64 => Math.ceil(x / step) * step;
