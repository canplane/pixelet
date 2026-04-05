// @pxl/spec/space/v3.ts

import type { f64, Vec } from "@xpute/spec/lib/kit/word.ts";

/** Fixed-length 3D float vector. */
export type V3 = Vec<3, f64>;

/** Fills all three lanes with the same scalar. */
export function fill3(e: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = out[1] = out[2] = e;
  return out;
}

export function set3(x: f64, y: f64, z: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = x;
  out[1] = y;
  out[2] = z;
  return out;
}

export function copy3(from: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = from[0];
  out[1] = from[1];
  out[2] = from[2];
  return out;
}

// ---- Arithmetic ----

export function neg3(v: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = -v[0];
  out[1] = -v[1];
  out[2] = -v[2];
  return out;
}
export function abs(v: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = Math.abs(v[0]);
  out[1] = Math.abs(v[1]);
  out[2] = Math.abs(v[2]);
  return out;
}

export function add3(v: Readonly<V3>, w: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] + w[0];
  out[1] = v[1] + w[1];
  out[2] = v[2] + w[2];
  return out;
}
export function add3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] + k;
  out[1] = v[1] + k;
  out[2] = v[2] + k;
  return out;
}
export function sub3(v: Readonly<V3>, w: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] - w[0];
  out[1] = v[1] - w[1];
  out[2] = v[2] - w[2];
  return out;
}
export function sub3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] - k;
  out[1] = v[1] - k;
  out[2] = v[2] - k;
  return out;
}
export function mul3(v: Readonly<V3>, w: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] * w[0];
  out[1] = v[1] * w[1];
  out[2] = v[2] * w[2];
  return out;
}
export function mul3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] * k;
  out[1] = v[1] * k;
  out[2] = v[2] * k;
  return out;
}
export function div3(v: Readonly<V3>, w: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] / w[0];
  out[1] = v[1] / w[1];
  out[2] = v[2] / w[2];
  return out;
}
export function div3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] / k;
  out[1] = v[1] / k;
  out[2] = v[2] / k;
  return out;
}
export function mod3(v: Readonly<V3>, w: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] % w[0];
  out[1] = v[1] % w[1];
  out[2] = v[2] % w[2];
  return out;
}
export function mod3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] % k;
  out[1] = v[1] % k;
  out[2] = v[2] % k;
  return out;
}

/** Floors each lane to the lower multiple of the corresponding span. */
export function floor3(base: Readonly<V3>, span: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = Math.floor(base[0] / span[0]) * span[0];
  out[1] = Math.floor(base[1] / span[1]) * span[1];
  out[2] = Math.floor(base[2] / span[2]) * span[2];
  return out;
}
export function floor3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = Math.floor(v[0] / k) * k;
  out[1] = Math.floor(v[1] / k) * k;
  out[2] = Math.floor(v[2] / k) * k;
  return out;
}
/** Returns the snapped cell center for `base` under `span`. */
export function center3(base: Readonly<V3>, span: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  floor3(base, span, out);
  out[0] += span[0] * 0.5;
  out[1] += span[1] * 0.5;
  out[2] += span[2] * 0.5;
  return out;
}
export function center3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  floor3n(v, k, out);
  const half = k * 0.5;
  out[0] += half;
  out[1] += half;
  out[2] += half;
  return out;
}
export function ceil3(base: Readonly<V3>, span: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = Math.ceil(base[0] / span[0]) * span[0];
  out[1] = Math.ceil(base[1] / span[1]) * span[1];
  out[2] = Math.ceil(base[2] / span[2]) * span[2];
  return out;
}
export function ceil3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = Math.ceil(v[0] / k) * k;
  out[1] = Math.ceil(v[1] / k) * k;
  out[2] = Math.ceil(v[2] / k) * k;
  return out;
}
export function round3(base: Readonly<V3>, span: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = Math.round(base[0] / span[0]) * span[0];
  out[1] = Math.round(base[1] / span[1]) * span[1];
  out[2] = Math.round(base[2] / span[2]) * span[2];
  return out;
}
export function round3n(v: Readonly<V3>, k: f64, out: V3 = [0, 0, 0]): V3 {
  out[0] = Math.round(v[0] / k) * k;
  out[1] = Math.round(v[1] / k) * k;
  out[2] = Math.round(v[2] / k) * k;
  return out;
}

export function i3(v: Readonly<V3>, out: V3 = [0, 0, 0]): V3 {
  out[0] = v[0] | 0;
  out[1] = v[1] | 0;
  out[2] = v[2] | 0;
  return out;
}

export function snap3n(v: Readonly<V3>, mask: number, out: V3 = [0, 0, 0]): V3 {
  out[0] = (v[0] | 0) & ~mask;
  out[1] = (v[1] | 0) & ~mask;
  out[2] = (v[2] | 0) & ~mask;
  return out;
}
