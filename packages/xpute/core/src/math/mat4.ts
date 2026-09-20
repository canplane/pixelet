// @xpute/core/math/mat4.ts

/**
 * 4x4 matrices as sixteen doubles, column-major — the layout GL, WebGPU and
 * glTF share, so a matrix here uploads as it is. Every function writes into
 * an `out` the caller owns; nothing allocates.
 */

import type { F64Array } from "../abi/array.ts";
import type { f64, u32 } from "../abi/word.ts";
import type { Vec3 } from "./vec3.ts";

export type Mat4 = Float64Array;

export const mat4 = (): Mat4 => mat4_identity(new Float64Array(16));

export function mat4_identity(out: Mat4): Mat4 {
  out.fill(0);
  out[0] =
    out[5] =
    out[10] =
    out[15] =
      1;
  return out;
}

/** out = a × b; `out` may be `a` or `b`. */
export function mat4_mul(out: Mat4, a: Mat4, b: Mat4): Mat4 {
  const a00 = a[0], a01 = a[1], a02 = a[2], a03 = a[3];
  const a10 = a[4], a11 = a[5], a12 = a[6], a13 = a[7];
  const a20 = a[8], a21 = a[9], a22 = a[10], a23 = a[11];
  const a30 = a[12], a31 = a[13], a32 = a[14], a33 = a[15];
  for (let c = 0; c < 4; c++) {
    const b0 = b[c * 4], b1 = b[c * 4 + 1], b2 = b[c * 4 + 2], b3 = b[c * 4 + 3];
    out[c * 4] = a00 * b0 + a10 * b1 + a20 * b2 + a30 * b3;
    out[c * 4 + 1] = a01 * b0 + a11 * b1 + a21 * b2 + a31 * b3;
    out[c * 4 + 2] = a02 * b0 + a12 * b1 + a22 * b2 + a32 * b3;
    out[c * 4 + 3] = a03 * b0 + a13 * b1 + a23 * b2 + a33 * b3;
  }
  return out;
}

/** The general inverse by cofactors; a singular matrix leaves `out` zero. */
export function mat4_invert(out: Mat4, m: Mat4): Mat4 {
  const n11 = m[0], n21 = m[1], n31 = m[2], n41 = m[3];
  const n12 = m[4], n22 = m[5], n32 = m[6], n42 = m[7];
  const n13 = m[8], n23 = m[9], n33 = m[10], n43 = m[11];
  const n14 = m[12], n24 = m[13], n34 = m[14], n44 = m[15];
  const t11 = n23 * n34 * n42 - n24 * n33 * n42 + n24 * n32 * n43 - n22 * n34 * n43 - n23 * n32 * n44 + n22 * n33 * n44;
  const t12 = n14 * n33 * n42 - n13 * n34 * n42 - n14 * n32 * n43 + n12 * n34 * n43 + n13 * n32 * n44 - n12 * n33 * n44;
  const t13 = n13 * n24 * n42 - n14 * n23 * n42 + n14 * n22 * n43 - n12 * n24 * n43 - n13 * n22 * n44 + n12 * n23 * n44;
  const t14 = n14 * n23 * n32 - n13 * n24 * n32 - n14 * n22 * n33 + n12 * n24 * n33 + n13 * n22 * n34 - n12 * n23 * n34;
  const det = n11 * t11 + n21 * t12 + n31 * t13 + n41 * t14;
  if (det === 0) return out.fill(0);
  const d = 1 / det;
  out[0] = t11 * d;
  out[1] = (n24 * n33 * n41 - n23 * n34 * n41 - n24 * n31 * n43 + n21 * n34 * n43 + n23 * n31 * n44 - n21 * n33 * n44) * d;
  out[2] = (n22 * n34 * n41 - n24 * n32 * n41 + n24 * n31 * n42 - n21 * n34 * n42 - n22 * n31 * n44 + n21 * n32 * n44) * d;
  out[3] = (n23 * n32 * n41 - n22 * n33 * n41 - n23 * n31 * n42 + n21 * n33 * n42 + n22 * n31 * n43 - n21 * n32 * n43) * d;
  out[4] = t12 * d;
  out[5] = (n13 * n34 * n41 - n14 * n33 * n41 + n14 * n31 * n43 - n11 * n34 * n43 - n13 * n31 * n44 + n11 * n33 * n44) * d;
  out[6] = (n14 * n32 * n41 - n12 * n34 * n41 - n14 * n31 * n42 + n11 * n34 * n42 + n12 * n31 * n44 - n11 * n32 * n44) * d;
  out[7] = (n12 * n33 * n41 - n13 * n32 * n41 + n13 * n31 * n42 - n11 * n33 * n42 - n12 * n31 * n43 + n11 * n32 * n43) * d;
  out[8] = t13 * d;
  out[9] = (n14 * n23 * n41 - n13 * n24 * n41 - n14 * n21 * n43 + n11 * n24 * n43 + n13 * n21 * n44 - n11 * n23 * n44) * d;
  out[10] = (n12 * n24 * n41 - n14 * n22 * n41 + n14 * n21 * n42 - n11 * n24 * n42 - n12 * n21 * n44 + n11 * n22 * n44) * d;
  out[11] = (n13 * n22 * n41 - n12 * n23 * n41 - n13 * n21 * n42 + n11 * n23 * n42 + n12 * n21 * n43 - n11 * n22 * n43) * d;
  out[12] = t14 * d;
  out[13] = (n13 * n24 * n31 - n14 * n23 * n31 + n14 * n21 * n33 - n11 * n24 * n33 - n13 * n21 * n34 + n11 * n23 * n34) * d;
  out[14] = (n14 * n22 * n31 - n12 * n24 * n31 - n14 * n21 * n32 + n11 * n24 * n32 + n12 * n21 * n34 - n11 * n22 * n34) * d;
  out[15] = (n12 * n23 * n31 - n13 * n22 * n31 + n13 * n21 * n32 - n11 * n23 * n32 - n12 * n21 * n33 + n11 * n22 * n33) * d;
  return out;
}

export function mat4_translation(out: Mat4, x: f64, y: f64, z: f64): Mat4 {
  mat4_identity(out);
  out[12] = x;
  out[13] = y;
  out[14] = z;
  return out;
}

export function mat4_rotation_z(out: Mat4, angle: f64): Mat4 {
  const c = Math.cos(angle), s = Math.sin(angle);
  mat4_identity(out);
  out[0] = c;
  out[1] = s;
  out[4] = -s;
  out[5] = c;
  return out;
}

/** GL's perspective: clip z in [-1, 1], `fov` the vertical angle in radians. */
export function mat4_perspective(out: Mat4, fov: f64, aspect: f64, near: f64, far: f64): Mat4 {
  const f = 1 / Math.tan(fov / 2);
  out.fill(0);
  out[0] = f / aspect;
  out[5] = f;
  out[10] = (far + near) / (near - far);
  out[11] = -1;
  out[14] = (2 * far * near) / (near - far);
  return out;
}

/** The view matrix of an eye at `eye` looking at `target` with `up` up:
 * world to camera, the camera looking down its own −Z. */
export function mat4_look_at(out: Mat4, eye: Vec3, target: Vec3, up: Vec3): Mat4 {
  let zx = eye.x - target.x, zy = eye.y - target.y, zz = eye.z - target.z;
  let l = Math.hypot(zx, zy, zz) || 1;
  zx /= l;
  zy /= l;
  zz /= l;
  let xx = up.y * zz - up.z * zy, xy = up.z * zx - up.x * zz, xz = up.x * zy - up.y * zx;
  l = Math.hypot(xx, xy, xz);
  if (l === 0) {
    // Looking straight along `up`: any horizontal right will do.
    xx = 1;
    xy = 0;
    xz = 0;
    l = 1;
  }
  xx /= l;
  xy /= l;
  xz /= l;
  const yx = zy * xz - zz * xy, yy = zz * xx - zx * xz, yz = zx * xy - zy * xx;
  out[0] = xx;
  out[4] = xy;
  out[8] = xz;
  out[12] = -(xx * eye.x + xy * eye.y + xz * eye.z);
  out[1] = yx;
  out[5] = yy;
  out[9] = yz;
  out[13] = -(yx * eye.x + yy * eye.y + yz * eye.z);
  out[2] = zx;
  out[6] = zy;
  out[10] = zz;
  out[14] = -(zx * eye.x + zy * eye.y + zz * eye.z);
  out[3] = out[7] = out[11] = 0;
  out[15] = 1;
  return out;
}

/** m × (x, y, z, 1), divided by w. */
export function mat4_transform_point(out: Vec3, m: Mat4, x: f64, y: f64, z: f64): Vec3 {
  const w = m[3] * x + m[7] * y + m[11] * z + m[15] || 1;
  out.x = (m[0] * x + m[4] * y + m[8] * z + m[12]) / w;
  out.y = (m[1] * x + m[5] * y + m[9] * z + m[13]) / w;
  out.z = (m[2] * x + m[6] * y + m[10] * z + m[14]) / w;
  return out;
}

/** m × (x, y, z, 0), normalized. */
export function mat4_transform_direction(out: Vec3, m: Mat4, x: f64, y: f64, z: f64): Vec3 {
  const ox = m[0] * x + m[4] * y + m[8] * z;
  const oy = m[1] * x + m[5] * y + m[9] * z;
  const oz = m[2] * x + m[6] * y + m[10] * z;
  const l = Math.hypot(ox, oy, oz) || 1;
  out.x = ox / l;
  out.y = oy / l;
  out.z = oz / l;
  return out;
}

/** The clip-space w of m × (x, y, z, 1): the point's depth for a projection. */
export function mat4_clip_w(m: Mat4, x: f64, y: f64, z: f64): f64 {
  return m[3] * x + m[7] * y + m[11] * z + m[15];
}

/**
 * The six planes of the frustum `view_proj` bounds, as (nx, ny, nz, d) with
 * the normals pointing in, normalized: a point is inside when
 * n · p + d ≥ 0 for every plane. Left, right, bottom, top, near, far.
 */
export function mat4_frustum_planes(out: F64Array, m: Mat4): F64Array {
  const rows = [
    [m[0], m[4], m[8], m[12]],
    [m[1], m[5], m[9], m[13]],
    [m[2], m[6], m[10], m[14]],
    [m[3], m[7], m[11], m[15]],
  ];
  const w = rows[3];
  const put = (k: u32, a: f64[], sign: 1 | -1) => {
    const nx = w[0] + sign * a[0], ny = w[1] + sign * a[1], nz = w[2] + sign * a[2], d = w[3] + sign * a[3];
    const l = Math.hypot(nx, ny, nz) || 1;
    out[k * 4] = nx / l;
    out[k * 4 + 1] = ny / l;
    out[k * 4 + 2] = nz / l;
    out[k * 4 + 3] = d / l;
  };
  put(0, rows[0], 1);
  put(1, rows[0], -1);
  put(2, rows[1], 1);
  put(3, rows[1], -1);
  put(4, rows[2], 1);
  put(5, rows[2], -1);
  return out;
}
