// pxl-spec/space/pvf3/projection.rs

//! Doubles in and out of PVF3, where 1.0 is one lattice unit. Each scalar is
//! split by `floor`, so the lattice stays continuous through zero: -0.2 is
//! cell -1 at 0.8.

use super::fvec;
use super::fvec::FVec;
use super::ivec;
use super::ivec::IVec;
use super::pvf3::Pvf3;
use crate::space::v3::V3;

pub fn project(x: f64, y: f64, z: f64) -> Pvf3 {
    Pvf3 {
        i: ivec::gather(ivec::from_real(x), ivec::from_real(y), ivec::from_real(z)),
        f: fvec::gather(fvec::from_real(x), fvec::from_real(y), fvec::from_real(z)),
    }
}

fn unproject_comp(i_comp: i64, f_comp: i64) -> f64 {
    ivec::to_real(i_comp) + fvec::to_real(f_comp)
}

pub fn unproject(i: IVec, f: FVec) -> V3 {
    [unproject_comp(ivec::X(i), fvec::X(f)), unproject_comp(ivec::Y(i), fvec::Y(f)), unproject_comp(ivec::Z(i), fvec::Z(f))]
}
