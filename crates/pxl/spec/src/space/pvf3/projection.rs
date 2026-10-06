// pxl-spec/space/pvf3/projection.rs

//! Doubles in and out of PVF3, where 1.0 is one lattice unit: a basis
//! transpose from each axis's integer and fraction, `[[x_i, x_f], [y_i, y_f],
//! [z_i, z_f]]`, to the lanes packed by kind, `{ i: [x_i, y_i, z_i], f: [x_f,
//! y_f, z_f] }`. Each scalar is split by `floor`, so the lattice stays
//! continuous through zero: -0.2 is cell -1 at 0.8.

use super::core::ONE;
use super::fvec;
use super::fvec::FVec;
use super::ivec;
use super::ivec::IVec;
use super::pvf3::Pvf3;
use super::REACH;
use crate::space::v3::V3;

/// The one door from doubles into PVF3: X goes round the world, Y and Z
/// stop at their lanes' ends (`REACH`, half-open).
pub fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    let [_, y, z] = within([x, y, z]);
    Pvf3 {
        i: ivec::gather(ivec::from_real(x), ivec::from_real(y), ivec::from_real(z)),
        f: fvec::gather(fvec::from_real(x), fvec::from_real(y), fvec::from_real(z)),
    }
}

/// `v` with Y and Z held inside `REACH`, as `project` stores them.
pub fn within(v: V3) -> V3 {
    let top = |k: usize| REACH.max[k] - 1.0 / ONE as f64;
    [v[0], v[1].clamp(REACH.min[1], top(1)), v[2].clamp(REACH.min[2], top(2))]
}

fn unproject_comp(i_comp: i64, f_comp: i64) -> f64 {
    ivec::to_real(i_comp) + fvec::to_real(f_comp)
}

pub fn unproject(i: IVec, f: FVec) -> V3 {
    [unproject_comp(ivec::X(i), fvec::X(f)), unproject_comp(ivec::Y(i), fvec::Y(f)), unproject_comp(ivec::Z(i), fvec::Z(f))]
}
