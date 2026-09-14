// script/vector/xpute/core/kit/fdlibm.ts
//
// V8's Math over inputs across every branch the fdlibm routines take —
// tiny, around pi/4 and the rem_pio2 cut-overs, huge, negative, the
// specials — for crates/xpute/core/src/kit/fdlibm.test.rs, which holds the
// Rust ports to the bit.

import { f64_bits, seeded } from "../../../../vector.ts";

export function vectors() {
  const rnd = seeded(0xfd1b);
  const u = () => rnd() / 0x1_0000_0000;
  const xs = [
    0,
    -0,
    1e-310,
    -1e-310,
    1e-20,
    2 ** -27,
    0.5,
    -0.5,
    0.49999999999999994,
    -0.49999999999999994,
    1,
    -1,
    Math.PI / 4,
    Math.PI / 2,
    Math.PI,
    3 * Math.PI / 4,
    2.5,
    -2.5,
    3.5,
    1e3,
    1e8,
    2 ** 20 * Math.PI,
    1e15,
    2 ** 52,
    2 ** 53 + 2,
    1e300,
    Infinity,
    -Infinity,
    NaN,
    709.78,
    -745.1,
    0.6931471805599453,
  ];
  for (let k = 0; k < 300; k++) xs.push((u() - 0.5) * 2 ** ((rnd() % 64) - 20));
  const ys = xs.slice(0, 40).reverse();
  return xs.map((x, k) => {
    const y = ys[k % ys.length];
    return {
      x: f64_bits(x),
      y: f64_bits(y),
      sin: f64_bits(Math.sin(x)),
      cos: f64_bits(Math.cos(x)),
      tan: f64_bits(Math.tan(x)),
      atan: f64_bits(Math.atan(x)),
      atan2: f64_bits(Math.atan2(y, x)),
      exp: f64_bits(Math.exp(x)),
      log: f64_bits(Math.log(x)),
      cbrt: f64_bits(Math.cbrt(x)),
      round: f64_bits(Math.round(x)),
      hypot: f64_bits(Math.hypot(x, y, x * 0.5)),
    };
  });
}
