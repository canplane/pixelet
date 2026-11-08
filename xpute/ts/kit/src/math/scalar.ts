// @xpute/kit/math/scalar.ts

import type { f64 } from "@xpute/kit/abi/word.ts";

export const EPS = 1e-5 as const;

/**
 * Equal within `eps`, by default `EPS`. The tolerance is absolute near zero
 * and relative to the larger magnitude beyond.
 */
export const approx_eq = (a: f64, b: f64, eps: f64 = EPS): boolean => {
  if (a === b) return true;
  const diff = Math.abs(a - b);
  return diff < eps || diff < eps * Math.max(Math.abs(a), Math.abs(b));
};

export const near = (a: f64, b: f64, tol: f64): boolean =>
  Math.abs(a - b) < tol;

/**
 * Rounds as JavaScript's `Math.round` does: half rounds up, so `-2.5`
 * becomes `-2`.
 */
export const round_half_up = (x: f64): f64 => Math.round(x);

/**
 * Rounds as C's and Rust's `round` do: half rounds away from zero, so `-2.5`
 * becomes `-3`.
 */
export const round_half_away = (x: f64): f64 =>
  Math.sign(x) * Math.round(Math.abs(x));

/**
 * Rounds as IEEE 754's default and WGSL's `round` do: half rounds to even, so
 * `2.5` becomes `2` and `-2.5` becomes `-2`.
 */
export const round_half_even = (x: f64): f64 => {
  const r = Math.round(x);
  return Math.abs(x - Math.trunc(x)) === 0.5 && r % 2 !== 0 ? r - 1 : r;
};

/**
 * The kit's one rounding, half to even as WGSL's `round`, so the CPU and a
 * shader land on the same integer. `Math.round` rounds half up instead.
 */
export const round = round_half_even;

export const lerp = (a: f64, b: f64, t: f64): f64 => a + (b - a) * t;
export const inv_lerp = (x: f64, min: f64, max: f64): f64 =>
  (x - min) / (max - min);

export const remap = (
  x: f64,
  in_min: f64,
  in_max: f64,
  out_min: f64,
  out_max: f64,
): f64 => lerp(out_min, out_max, inv_lerp(x, in_min, in_max));

export const clamp = (x: f64, min: f64, max: f64): f64 =>
  x < min ? min : x > max ? max : x;
export const wrap = (x: f64, min: f64, max: f64): f64 => {
  const range = max - min;
  return ((((x - min) % range) + range) % range) + min;
};

export const floor_to_step = (x: f64, step: f64): f64 =>
  Math.floor(x / step) * step;
export const round_to_step = (x: f64, step: f64): f64 => round(x / step) * step;
export const ceil_to_step = (x: f64, step: f64): f64 =>
  Math.ceil(x / step) * step;
