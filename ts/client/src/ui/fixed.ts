// @pxl/client/ui/fixed.ts

import { round } from "@xpute/kit/math/scalar.ts";

/**
 * `x` written with `digits` places after the point, its last place rounded
 * half to even, as every rounding in the tree is (xpute/ARCHITECTURE.md,
 * `math/scalar`). It prints what `toFixed` prints except at a tie, which
 * `toFixed` rounds up. The digits are the kit's `round` of `x` scaled to an
 * integer, with the point set into them, so that one rule holds on every
 * runtime.
 */
export function fixed(x: number, digits: number): string {
  if (!Number.isFinite(x)) return String(x);
  const whole = String(round(Math.abs(x) * 10 ** digits)).padStart(
    digits + 1,
    "0",
  );
  const at = whole.length - digits;
  const sign = x < 0 && /[1-9]/.test(whole) ? "-" : "";
  return digits === 0
    ? `${sign}${whole}`
    : `${sign}${whole.slice(0, at)}.${whole.slice(at)}`;
}
