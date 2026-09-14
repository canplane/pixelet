// script/vector/xpute/core/kit/math.ts
//
// kit/math.ts over halves, negatives and steps, for
// crates/xpute/core/src/kit/math.test.rs.

import * as m from "@xpute/core/kit/math.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  const xs = [0, -0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 0.49999999999999994, 7.3, -7.3, 1e15 + 0.5, -359.9, 720.25];
  const steps = [1, 0.5, 3, 0.1];
  return xs.flatMap((x) =>
    steps.map((s) => ({
      x: f64_bits(x),
      s: f64_bits(s),
      approx_eq: m.approx_eq(x, x + s * 1e-6),
      near: m.near(x, s, 0.3),
      lerp: f64_bits(m.lerp(x, s, 0.3)),
      inv_lerp: f64_bits(m.inv_lerp(x, -s, s * 2)),
      remap: f64_bits(m.remap(x, -1, 1, 0, s)),
      clamp: f64_bits(m.clamp(x, -s, s)),
      wrap: f64_bits(m.wrap(x, -180, 180)),
      floor_to_step: f64_bits(m.floor_to_step(x, s)),
      round_to_step: f64_bits(m.round_to_step(x, s)),
      ceil_to_step: f64_bits(m.ceil_to_step(x, s)),
    }))
  );
}
