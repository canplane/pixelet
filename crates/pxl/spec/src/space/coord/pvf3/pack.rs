// pxl-spec/space/coord/pvf3/pack.rs

//! Doubles in and out of PVF3, where 1.0 is one lattice unit: a basis
//! transpose from each axis's integer and fraction, `[[x_i, x_f], [y_i, y_f],
//! [z_i, z_f]]`, to the lanes packed by kind, `{ i: [x_i, y_i, z_i], f: [x_f,
//! y_f, z_f] }`. Each scalar is split by `floor`, so the lattice stays
//! continuous through zero: -0.2 is cell -1 at 0.8. A value past a lane's
//! width wraps in it.

use super::fvec;
use super::fvec::FVec;
use super::ivec;
use super::ivec::IVec;
use super::pvf3::Pvf3;
use crate::space::coord::v3::V3;

pub fn pack([x, y, z]: V3) -> Pvf3 {
    Pvf3 {
        i: ivec::gather(ivec::from_real(x), ivec::from_real(y), ivec::from_real(z)),
        f: fvec::gather(fvec::from_real(x), fvec::from_real(y), fvec::from_real(z)),
    }
}

fn unpack_comp(i_comp: i64, f_comp: i64) -> f64 {
    ivec::to_real(i_comp) + fvec::to_real(f_comp)
}

pub fn unpack(i: IVec, f: FVec) -> V3 {
    [unpack_comp(ivec::X(i), fvec::X(f)), unpack_comp(ivec::Y(i), fvec::Y(f)), unpack_comp(ivec::Z(i), fvec::Z(f))]
}
