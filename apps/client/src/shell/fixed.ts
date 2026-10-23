// @pxl/client/shell/fixed.ts

import { round } from "@xpute/kit/math/scalar.ts";

/**
 * `x` with `digits` places after the point, its last rounded half to even as
 * every rounding here is (ARCHITECTURE.md, Terrain): what `toFixed` would
 * print but for its ties, which it rounds up. The digits are the integer
 * `round` makes of `x` scaled, the point set into them, so the one rule is the
 * kit's on every runtime.
 */
export function fixed(x: number, digits: number): string {
  if (!Number.isFinite(x)) return String(x);
  const whole = String(round(Math.abs(x) * 10 ** digits)).padStart(digits + 1, "0");
  const at = whole.length - digits;
  const sign = x < 0 && /[1-9]/.test(whole) ? "-" : "";
  return digits === 0 ? `${sign}${whole}` : `${sign}${whole.slice(0, at)}.${whole.slice(at)}`;
}
