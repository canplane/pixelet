// xpute-core/math/mat4.rs

//! 4x4 matrices as sixteen doubles, column-major — the layout GL, WebGPU and
//! glTF share, so a matrix here uploads as it is. Every function writes into
//! an `out` the caller owns; nothing allocates. The trigonometry and the
//! lengths are `libm`'s, so the doubles are the same on every target.

use super::vec3::Vec3;

pub type Mat4 = [f64; 16];

pub fn mat4() -> Mat4 {
    let mut out = [0.0; 16];
    mat4_identity(&mut out);
    out
}

pub fn mat4_identity(out: &mut Mat4) -> &mut Mat4 {
    out.fill(0.0);
    out[0] = 1.0;
    out[5] = 1.0;
    out[10] = 1.0;
    out[15] = 1.0;
    out
}

/// out = a × b; `out` may be `a` or `b`.
pub fn mat4_mul<'a>(out: &'a mut Mat4, a: &Mat4, b: &Mat4) -> &'a mut Mat4 {
    let (a00, a01, a02, a03) = (a[0], a[1], a[2], a[3]);
    let (a10, a11, a12, a13) = (a[4], a[5], a[6], a[7]);
    let (a20, a21, a22, a23) = (a[8], a[9], a[10], a[11]);
    let (a30, a31, a32, a33) = (a[12], a[13], a[14], a[15]);
    for c in 0..4 {
        let (b0, b1, b2, b3) = (b[c * 4], b[c * 4 + 1], b[c * 4 + 2], b[c * 4 + 3]);
        out[c * 4] = a00 * b0 + a10 * b1 + a20 * b2 + a30 * b3;
        out[c * 4 + 1] = a01 * b0 + a11 * b1 + a21 * b2 + a31 * b3;
        out[c * 4 + 2] = a02 * b0 + a12 * b1 + a22 * b2 + a32 * b3;
        out[c * 4 + 3] = a03 * b0 + a13 * b1 + a23 * b2 + a33 * b3;
    }
    out
}

/// The general inverse by cofactors; a singular matrix leaves `out` zero.
pub fn mat4_invert<'a>(out: &'a mut Mat4, m: &Mat4) -> &'a mut Mat4 {
    let (n11, n21, n31, n41) = (m[0], m[1], m[2], m[3]);
    let (n12, n22, n32, n42) = (m[4], m[5], m[6], m[7]);
    let (n13, n23, n33, n43) = (m[8], m[9], m[10], m[11]);
    let (n14, n24, n34, n44) = (m[12], m[13], m[14], m[15]);
    let t11 = n23 * n34 * n42 - n24 * n33 * n42 + n24 * n32 * n43 - n22 * n34 * n43 - n23 * n32 * n44 + n22 * n33 * n44;
    let t12 = n14 * n33 * n42 - n13 * n34 * n42 - n14 * n32 * n43 + n12 * n34 * n43 + n13 * n32 * n44 - n12 * n33 * n44;
    let t13 = n13 * n24 * n42 - n14 * n23 * n42 + n14 * n22 * n43 - n12 * n24 * n43 - n13 * n22 * n44 + n12 * n23 * n44;
    let t14 = n14 * n23 * n32 - n13 * n24 * n32 - n14 * n22 * n33 + n12 * n24 * n33 + n13 * n22 * n34 - n12 * n23 * n34;
    let det = n11 * t11 + n21 * t12 + n31 * t13 + n41 * t14;
    if det == 0.0 {
        out.fill(0.0);
        return out;
    }
    let d = 1.0 / det;
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
    out
}

pub fn mat4_translation(out: &mut Mat4, x: f64, y: f64, z: f64) -> &mut Mat4 {
    mat4_identity(out);
    out[12] = x;
    out[13] = y;
    out[14] = z;
    out
}

pub fn mat4_rotation_z(out: &mut Mat4, angle: f64) -> &mut Mat4 {
    let (c, s) = (libm::cos(angle), libm::sin(angle));
    mat4_identity(out);
    out[0] = c;
    out[1] = s;
    out[4] = -s;
    out[5] = c;
    out
}

/// GL's perspective: clip z in [-1, 1], `fov` the vertical angle in radians.
pub fn mat4_perspective(out: &mut Mat4, fov: f64, aspect: f64, near: f64, far: f64) -> &mut Mat4 {
    let f = 1.0 / libm::tan(fov / 2.0);
    out.fill(0.0);
    out[0] = f / aspect;
    out[5] = f;
    out[10] = (far + near) / (near - far);
    out[11] = -1.0;
    out[14] = (2.0 * far * near) / (near - far);
    out
}

/// `x || 1` on a length: 1 where the length is 0 or NaN, both falsy.
fn or_one(l: f64) -> f64 {
    if l == 0.0 || l.is_nan() {
        1.0
    } else {
        l
    }
}

/// The view matrix of an eye at `eye` looking at `target` with `up` up:
/// world to camera, the camera looking down its own −Z.
pub fn mat4_look_at(out: &mut Mat4, eye: Vec3, target: Vec3, up: Vec3) -> &mut Mat4 {
    let (mut zx, mut zy, mut zz) = (eye.x - target.x, eye.y - target.y, eye.z - target.z);
    let mut l = or_one(libm::hypot(libm::hypot(zx, zy), zz));
    zx /= l;
    zy /= l;
    zz /= l;
    let (mut xx, mut xy, mut xz) = (up.y * zz - up.z * zy, up.z * zx - up.x * zz, up.x * zy - up.y * zx);
    l = libm::hypot(libm::hypot(xx, xy), xz);
    if l == 0.0 {
        // Looking straight along `up`: any horizontal right will do.
        xx = 1.0;
        xy = 0.0;
        xz = 0.0;
        l = 1.0;
    }
    xx /= l;
    xy /= l;
    xz /= l;
    let (yx, yy, yz) = (zy * xz - zz * xy, zz * xx - zx * xz, zx * xy - zy * xx);
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
    out[3] = 0.0;
    out[7] = 0.0;
    out[11] = 0.0;
    out[15] = 1.0;
    out
}

/// m × (x, y, z, 1), divided by w.
pub fn mat4_transform_point<'a>(out: &'a mut Vec3, m: &Mat4, x: f64, y: f64, z: f64) -> &'a mut Vec3 {
    let w = or_one(m[3] * x + m[7] * y + m[11] * z + m[15]);
    out.x = (m[0] * x + m[4] * y + m[8] * z + m[12]) / w;
    out.y = (m[1] * x + m[5] * y + m[9] * z + m[13]) / w;
    out.z = (m[2] * x + m[6] * y + m[10] * z + m[14]) / w;
    out
}

/// m × (x, y, z, 0), normalized.
pub fn mat4_transform_direction<'a>(out: &'a mut Vec3, m: &Mat4, x: f64, y: f64, z: f64) -> &'a mut Vec3 {
    let ox = m[0] * x + m[4] * y + m[8] * z;
    let oy = m[1] * x + m[5] * y + m[9] * z;
    let oz = m[2] * x + m[6] * y + m[10] * z;
    let l = or_one(libm::hypot(libm::hypot(ox, oy), oz));
    out.x = ox / l;
    out.y = oy / l;
    out.z = oz / l;
    out
}

/// The clip-space w of m × (x, y, z, 1): the point's depth for a projection.
pub fn mat4_clip_w(m: &Mat4, x: f64, y: f64, z: f64) -> f64 {
    m[3] * x + m[7] * y + m[11] * z + m[15]
}

/// The six planes of the frustum `view_proj` bounds, as (nx, ny, nz, d) with
/// the normals pointing in, normalized: a point is inside when
/// n · p + d ≥ 0 for every plane. Left, right, bottom, top, near, far.
pub fn mat4_frustum_planes<'a>(out: &'a mut [f64; 24], m: &Mat4) -> &'a mut [f64; 24] {
    let rows = [[m[0], m[4], m[8], m[12]], [m[1], m[5], m[9], m[13]], [m[2], m[6], m[10], m[14]], [m[3], m[7], m[11], m[15]]];
    let w = rows[3];
    let mut put = |k: usize, a: [f64; 4], sign: f64| {
        let (nx, ny, nz, d) = (w[0] + sign * a[0], w[1] + sign * a[1], w[2] + sign * a[2], w[3] + sign * a[3]);
        let l = or_one(libm::hypot(libm::hypot(nx, ny), nz));
        out[k * 4] = nx / l;
        out[k * 4 + 1] = ny / l;
        out[k * 4 + 2] = nz / l;
        out[k * 4 + 3] = d / l;
    };
    put(0, rows[0], 1.0);
    put(1, rows[0], -1.0);
    put(2, rows[1], 1.0);
    put(3, rows[1], -1.0);
    put(4, rows[2], 1.0);
    put(5, rows[2], -1.0);
    out
}

#[cfg(test)]
#[path = "mat4.test.rs"]
mod test;
