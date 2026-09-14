// script/vector/xpute/core/kit/mat4.ts
//
// Every mat4 function over fixed matrices, for
// crates/xpute/core/src/kit/mat4.test.rs. What passes through Math.cos,
// sin, tan or hypot is held to a few ulps there; the arithmetic alone to
// the bit.

import * as m from "@xpute/core/kit/mat4.ts";
import { f64_bits } from "../../../../vector.ts";

export function vectors() {
  const B = (a: ArrayLike<number>) => Array.from(a, f64_bits);
  const r = m.mat4_rotation_z(m.mat4(), 0.7);
  const t = m.mat4_translation(m.mat4(), 5, -3, 2.25);
  const persp = m.mat4_perspective(m.mat4(), Math.PI / 3, 1.6, 0.1, 1000);
  const look = m.mat4_look_at(m.mat4(), { x: 0.5, y: -30, z: 40 }, { x: 1, y: 2, z: 0 }, { x: 0, y: 0, z: 1 });
  const up = m.mat4_look_at(m.mat4(), { x: 0, y: 0, z: 10 }, { x: 0, y: 0, z: 0 }, { x: 0, y: 0, z: 1 });
  const vp = m.mat4_mul(m.mat4(), persp, look);
  const p = m.mat4_transform_point({ x: 0, y: 0, z: 0 }, vp, 3, -4, 5);
  const d = m.mat4_transform_direction({ x: 0, y: 0, z: 0 }, look, 1, 2, 3);
  return {
    rotation_z: B(r),
    translation: B(t),
    mul: B(m.mat4_mul(m.mat4(), r, t)),
    perspective: B(persp),
    look_at: B(look),
    look_up: B(up),
    vp: B(vp),
    invert: B(m.mat4_invert(m.mat4(), vp)),
    singular: B(m.mat4_invert(m.mat4(), new Float64Array(16))),
    point: [p.x, p.y, p.z].map(f64_bits),
    direction: [d.x, d.y, d.z].map(f64_bits),
    clip_w: f64_bits(m.mat4_clip_w(vp, 3, -4, 5)),
    planes: B(m.mat4_frustum_planes(new Float64Array(24), vp)),
  };
}
