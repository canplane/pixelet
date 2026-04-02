// @pxl/spec/space/v3.ts

import type { f64, Vec } from "@xpute/spec/lib/kit/word.ts";

/** Fixed-length 3D float vector. */
export type V3 = Vec<3, f64>;

/** Fills all three lanes with the same scalar. */
export function fill3(e: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = out[1] = out[2] = e;
  return out;
}

/** Floors each lane to the lower multiple of the corresponding span. */
export function floor3(base: Readonly<V3>, span: Readonly<V3> | f64, out: V3 = [0, 0, 0]): V3 {
  if (typeof span === "number") span = fill3(span);
  out[0] = Math.floor(base[0] / span[0]) * span[0];
  out[1] = Math.floor(base[1] / span[1]) * span[1];
  out[2] = Math.floor(base[2] / span[2]) * span[2];
  return out;
}

/** Returns the snapped cell center for `base` under `span`. */
export function center3(base: Readonly<V3>, span: Readonly<V3> | f64, out: V3 = [0, 0, 0]): V3 {
  if (typeof span === "number") span = fill3(span);
  floor3(base, span, out);
  out[0] += span[0] * 0.5;
  out[1] += span[1] * 0.5;
  out[2] += span[2] * 0.5;
  return out;
}

/** Adds two 3D vectors lane-wise. */
export function add3(v: Readonly<V3>, w: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] + w[0];
  out[1] = v[1] + w[1];
  out[2] = v[2] + w[2];
  return out;
}
