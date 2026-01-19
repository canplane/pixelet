// @pxl/spec/space/v3.ts

import type { f64, Vec } from "@xpute/spec/lib/kit/word.ts";

export type V3 = Vec<3, f64>;

export const UNIT = [1, 1, 1] as const;

export function floor3(base: Readonly<V3>, span: Readonly<V3>): V3 {
  return [
    Math.floor(base[0] / span[0]) * span[0],
    Math.floor(base[1] / span[1]) * span[1],
    Math.floor(base[2] / span[2]) * span[2],
  ];
}

/** Snap XYZ to lattice cell span centers. */
export function center3(base: Readonly<V3>, span: Readonly<V3>): V3 {
  const p = floor3(base, span);
  return [
    p[0] + span[0] * 0.5,
    p[1] + span[1] * 0.5,
    p[2] + span[2] * 0.5,
  ];
}

export function add3(v: Readonly<V3>, w: Readonly<V3>): V3 {
  const x: f64 = v[0] + w[0];
  const y: f64 = v[1] + w[1];
  const z: f64 = v[2] + w[2];
  return [x, y, z];
}

export function fill3(e: f64): V3 {
  return [e, e, e];
}
