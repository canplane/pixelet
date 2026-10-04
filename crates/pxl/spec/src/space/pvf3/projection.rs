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

fn project_comp(reg: &mut [i64; 6], axis: usize, comp: f64) {
    reg[2 * axis] = ivec::from_real(comp);
    reg[2 * axis + 1] = fvec::from_real(comp);
}

pub fn project(out: &mut Pvf3, x: f64, y: f64, z: f64) -> &mut Pvf3 {
    // [x_i, x_f, y_i, y_f, z_i, z_f]
    let mut reg = [0i64; 6];
    project_comp(&mut reg, 0, x);
    project_comp(&mut reg, 1, y);
    project_comp(&mut reg, 2, z);
    out.i = ivec::gather(reg[0], reg[2], reg[4]);
    out.f = fvec::gather(reg[1], reg[3], reg[5]);
    out
}

fn unproject_comp(i_comp: i64, f_comp: i64) -> f64 {
    ivec::to_real(i_comp) + fvec::to_real(f_comp)
}

pub fn unproject(out: &mut V3, i: IVec, f: FVec) -> &mut V3 {
    out[0] = unproject_comp(ivec::X(i), fvec::X(f));
    out[1] = unproject_comp(ivec::Y(i), fvec::Y(f));
    out[2] = unproject_comp(ivec::Z(i), fvec::Z(f));
    out
}
