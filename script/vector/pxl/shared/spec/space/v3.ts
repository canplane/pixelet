// script/vector/pxl/shared/spec/space/v3.ts
//
// Every V3 helper over fixed vectors, halves and negatives included, for
// crates/pxl/shared/src/spec/space/v3.test.rs.

import * as v3 from "@pxl/shared/spec/space/v3.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import { f64_bits } from "../../../../../vector.ts";

export function vectors() {
  const vs: V3[] = [[0, 0, 0], [1.5, -2.5, 3.25], [-0.5, 0.5, -1.5], [2.5, 3.5, -4.5], [1e10, -1e-10, 7], [-3, 4, 12], [0.49999999999999994, -0.49999999999999994, 2147483648.7]];
  const B = (v: V3) => v.map(f64_bits);
  const pairs = vs.flatMap((a) => vs.map((b) => [a, b] as const));
  return {
    unary: vs.map((v) => ({
      v: B(v),
      neg3: B(v3.neg3(v)),
      abs3: B(v3.abs3(v)),
      norm3: B(v3.norm3(v)),
      i3: B(v3.i3(v)),
      snap3n: B(v3.snap3n(v, 7)),
      add3n: B(v3.add3n(v, 0.25)),
      sub3n: B(v3.sub3n(v, 0.25)),
      mul3n: B(v3.mul3n(v, -3)),
      div3n: B(v3.div3n(v, 7)),
      mod3n: B(v3.mod3n(v, 2)),
      floor3n: B(v3.floor3n(v, 2)),
      center3n: B(v3.center3n(v, 2)),
      ceil3n: B(v3.ceil3n(v, 2)),
      round3n: B(v3.round3n(v, 1)),
    })),
    binary: pairs.map(([a, b]) => ({
      a: B(a),
      b: B(b),
      add3: B(v3.add3(a, b)),
      sub3: B(v3.sub3(a, b)),
      mul3: B(v3.mul3(a, b)),
      div3: B(v3.div3(a, b)),
      mod3: B(v3.mod3(a, b)),
      dot3: f64_bits(v3.dot3(a, b)),
      cross3: B(v3.cross3(a, b)),
      floor3: B(v3.floor3(a, b)),
      center3: B(v3.center3(a, b)),
      ceil3: B(v3.ceil3(a, b)),
      round3: B(v3.round3(a, b)),
    })),
  };
}
