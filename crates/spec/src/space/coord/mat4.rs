// pxl-spec/space/coord/mat4.rs

//! 4x4 matrices as sixteen doubles, column-major as GL, WebGPU and glTF
//! upload them. Trigonometry is `libm`'s, so results match on every target.

use super::v3::V3;

pub type Mat4 = [f64; 16];

pub fn new() -> Mat4 {
    let mut out = [0.0; 16];
    identity(&mut out);
    out
}

pub fn identity(out: &mut Mat4) -> &mut Mat4 {
    out.fill(0.0);
    out[0] = 1.0;
    out[5] = 1.0;
    out[10] = 1.0;
    out[15] = 1.0;
    out
}

/// out = a × b; `out` may be `a` or `b`.
pub fn mul<'a>(out: &'a mut Mat4, a: &Mat4, b: &Mat4) -> &'a mut Mat4 {
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
pub fn invert<'a>(out: &'a mut Mat4, m: &Mat4) -> &'a mut Mat4 {
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

pub fn translation(out: &mut Mat4, x: f64, y: f64, z: f64) -> &mut Mat4 {
    identity(out);
    out[12] = x;
    out[13] = y;
    out[14] = z;
    out
}

pub fn rotation_z(out: &mut Mat4, angle: f64) -> &mut Mat4 {
    let (c, s) = (libm::cos(angle), libm::sin(angle));
    identity(out);
    out[0] = c;
    out[1] = s;
    out[4] = -s;
    out[5] = c;
    out
}

/// The rotation a quaternion (x, y, z, w) turns by, normalized first.
pub fn rotation_quat(out: &mut Mat4, [x, y, z, w]: [f64; 4]) -> &mut Mat4 {
    let n = libm::sqrt(x * x + y * y + z * z + w * w);
    let (x, y, z, w) = (x / n, y / n, z / n, w / n);
    identity(out);
    out[0] = 1.0 - 2.0 * (y * y + z * z);
    out[1] = 2.0 * (x * y + z * w);
    out[2] = 2.0 * (x * z - y * w);
    out[4] = 2.0 * (x * y - z * w);
    out[5] = 1.0 - 2.0 * (x * x + z * z);
    out[6] = 2.0 * (y * z + x * w);
    out[8] = 2.0 * (x * z + y * w);
    out[9] = 2.0 * (y * z - x * w);
    out[10] = 1.0 - 2.0 * (x * x + y * y);
    out
}

/// A rotation's inverse, exactly: its transpose.
pub fn transpose<'a>(out: &'a mut Mat4, m: &Mat4) -> &'a mut Mat4 {
    for c in 0..4 {
        for r in 0..4 {
            out[c * 4 + r] = m[r * 4 + c];
        }
    }
    out
}

/// GL's perspective: clip z in [-1, 1], `fov` the vertical angle in radians.
pub fn perspective(out: &mut Mat4, fov: f64, aspect: f64, near: f64, far: f64) -> &mut Mat4 {
    let f = 1.0 / libm::tan(fov / 2.0);
    out.fill(0.0);
    out[0] = f / aspect;
    out[5] = f;
    out[10] = (far + near) / (near - far);
    out[11] = -1.0;
    out[14] = (2.0 * far * near) / (near - far);
    out
}

fn or_one(l: f64) -> f64 {
    if l == 0.0 || l.is_nan() {
        1.0
    } else {
        l
    }
}

/// The view matrix of an eye at `eye` looking at `target` with `up` up:
/// world to camera, the camera looking down its own −Z.
pub fn look_at(out: &mut Mat4, eye: V3, target: V3, up: V3) -> &mut Mat4 {
    let (mut zx, mut zy, mut zz) = (eye[0] - target[0], eye[1] - target[1], eye[2] - target[2]);
    let mut l = or_one(libm::hypot(libm::hypot(zx, zy), zz));
    zx /= l;
    zy /= l;
    zz /= l;
    let (mut xx, mut xy, mut xz) = (up[1] * zz - up[2] * zy, up[2] * zx - up[0] * zz, up[0] * zy - up[1] * zx);
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
    out[12] = -(xx * eye[0] + xy * eye[1] + xz * eye[2]);
    out[1] = yx;
    out[5] = yy;
    out[9] = yz;
    out[13] = -(yx * eye[0] + yy * eye[1] + yz * eye[2]);
    out[2] = zx;
    out[6] = zy;
    out[10] = zz;
    out[14] = -(zx * eye[0] + zy * eye[1] + zz * eye[2]);
    out[3] = 0.0;
    out[7] = 0.0;
    out[11] = 0.0;
    out[15] = 1.0;
    out
}

pub fn transform_point<'a>(out: &'a mut V3, m: &Mat4, x: f64, y: f64, z: f64) -> &'a mut V3 {
    let w = or_one(m[3] * x + m[7] * y + m[11] * z + m[15]);
    out[0] = (m[0] * x + m[4] * y + m[8] * z + m[12]) / w;
    out[1] = (m[1] * x + m[5] * y + m[9] * z + m[13]) / w;
    out[2] = (m[2] * x + m[6] * y + m[10] * z + m[14]) / w;
    out
}

pub fn transform_direction<'a>(out: &'a mut V3, m: &Mat4, x: f64, y: f64, z: f64) -> &'a mut V3 {
    let ox = m[0] * x + m[4] * y + m[8] * z;
    let oy = m[1] * x + m[5] * y + m[9] * z;
    let oz = m[2] * x + m[6] * y + m[10] * z;
    let l = or_one(libm::hypot(libm::hypot(ox, oy), oz));
    out[0] = ox / l;
    out[1] = oy / l;
    out[2] = oz / l;
    out
}

/// The clip-space w of m × (x, y, z, 1): the point's depth for a projection.
pub fn clip_w(m: &Mat4, x: f64, y: f64, z: f64) -> f64 {
    m[3] * x + m[7] * y + m[11] * z + m[15]
}

/// The frustum's six planes (nx, ny, nz, d), normals in, normalized: left,
/// right, bottom, top, near, far.
pub fn frustum_planes<'a>(out: &'a mut [f64; 24], m: &Mat4) -> &'a mut [f64; 24] {
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
