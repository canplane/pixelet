// pxl-render/gpu/put.rs

//! Writes matrices into the words of the buffers the device is handed.

use pxl_spec::space::coord::mat4::Mat4;

pub(crate) fn put_mat(out: &mut [f32], at: usize, m: &Mat4) {
    for (i, v) in m.iter().enumerate() {
        out[at + i] = *v as f32;
    }
}

/// Writes at `at`, as the columns of a `Mat3`, the world step that moves an orthographic box's
/// clip space by one unit along each of its first three axes: row `r` of `m` divided by its
/// squared length. Each is computed in f32, as a shader reads `m`.
pub(crate) fn put_steps(out: &mut [f32], at: usize, m: &Mat4) {
    for r in 0..3 {
        let row = [m[r] as f32, m[4 + r] as f32, m[8 + r] as f32];
        let l = row[0] * row[0] + row[1] * row[1] + row[2] * row[2];
        for (o, v) in out[at + 4 * r..at + 4 * r + 3].iter_mut().zip(row) {
            *o = v / l;
        }
    }
}
