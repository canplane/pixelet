// @pxl/spec/space/pvf3/projection.rs
//
// PVF3 Normalized Scalar <-> PVF3 Projection.
//
// Performs a basis transpose between:
// - axis-paired form : [[x_i, x_f], [y_i, y_f], [z_i, z_f]]
// - lane-packed form : PVF3 { i: IVec, f: FVec }
//
// project:   (x, y, z: f64) -> PVF3   (floor decomposition, continuity across negative domain)
// unproject: (i: IVec, f: FVec) -> [f64; 3]

use super::{fvec, fvec::FVec, ivec, ivec::IVec, pvf3::Pvf3};

/// Decomposes (x, y, z) in normalized lattice units into PVF3.
///
/// Uses floor() so that decomposition is continuous across negative domain:
///   -0.2 => i = -1, f = 0.8
pub fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    Pvf3 {
        i: ivec::gather(ivec::from_real(x), ivec::from_real(y), ivec::from_real(z)),
        f: fvec::gather(fvec::from_real(x), fvec::from_real(y), fvec::from_real(z)),
    }
}

/// Reconstructs normalized (x, y, z) from packed integer and fractional lanes.
pub fn unproject(i: IVec, f: FVec) -> [f64; 3] {
    [
        ivec::to_real(ivec::x(i)) + fvec::to_real(fvec::x(f)),
        ivec::to_real(ivec::y(i)) + fvec::to_real(fvec::y(f)),
        ivec::to_real(ivec::z(i)) + fvec::to_real(fvec::z(f)),
    ]
}
