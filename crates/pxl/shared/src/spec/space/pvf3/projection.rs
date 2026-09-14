// pxl_shared/spec/space/pvf3/projection.rs

//! PVF3 (Packed 3-Vector w/ Fixed-Point Arithmetic)
//! Normalized Scalar <-> PVF3 Projection
//!
//! Provides generalized mapping functions for projecting continuous floating-point
//! values into hierarchical discrete layers (Integer Index + Fractional Offset)
//! and recovering them back into the scalar domain.
//!
//! Design Philosophy:
//! - Domain Agnostic: Operates on pure Euclidean scalar space. Makes no assumptions
//!   about coordinate systems or physical units.
//! - Hierarchical Consistency: Preserves lattice continuity across negative domains.
//! - Mathematical Determinism: Bridges IEEE 754 continuity and BigInt-packed precision.
//!
//! TERMINOLOGY
//! -----------
//! This module operates in a *normalized scalar space*:
//! - 1.0 means "one lattice unit" (caller defines what that unit is).
//! - Scaling to/from meters (or any physical unit) is done outside.
//!
//! IDENTITY
//! --------
//! This module performs a basis transpose between:
//! - axis-paired form : [[x_i, x_f], [y_i, y_f], [z_i, z_f]]
//! - lane-packed form : [[x_i, y_i, z_i], [x_f, y_f, z_f]] == { i, f }
//!
//! In other words, it factors a scalar vector into:
//! - integer lattice lanes (IVec) and
//! - fractional remainder lanes (FVec),
//!
//! and packs them into PVF3.
//!
//! The module-level register the TypeScript projects through is a local
//! here, since a projection owns it for the call.

use super::fvec;
use super::fvec::FVec;
use super::ivec;
use super::ivec::IVec;
use super::pvf3::PVF3;
use crate::spec::space::v3::V3;

/// Scalar projection.
///
/// Decomposes one normalized scalar into:
/// - i : integer lattice lane
/// - f : fractional residue lane
///
/// Notes:
/// - Uses floor() to preserve continuity across negative domains.
///   Example: -0.2 => i = -1, f = 0.8
fn project_comp(reg: &mut [i64; 6], axis: usize, comp: f64) {
    // i_comp: integer lattice position (stable across negative domain)
    reg[2 * axis] = ivec::from_real(comp);
    // f_comp: map fractional remainder into fixed-point lane resolution
    reg[2 * axis + 1] = fvec::from_real(comp);
}

/// Vectorized projection.
///
/// Converts (x,y,z) in normalized lattice units into PVF3:
/// - i : packed integer lanes
/// - f : packed fractional residue lanes
pub fn project(out: &mut PVF3, x: f64, y: f64, z: f64) -> &mut PVF3 {
    // [x_i, x_f, y_i, y_f, z_i, z_f]
    let mut reg = [0i64; 6];
    project_comp(&mut reg, 0, x);
    project_comp(&mut reg, 1, y);
    project_comp(&mut reg, 2, z);
    out.i = ivec::gather(reg[0], reg[2], reg[4]);
    out.f = fvec::gather(reg[1], reg[3], reg[5]);
    out
}

/// Reconstructs a normalized scalar from:
/// - i_comp : integer lattice index
/// - f_comp : fractional remainder (fixed-point lane)
///
/// Returns the normalized coordinate in "lattice units".
fn unproject_comp(i_comp: i64, f_comp: i64) -> f64 {
    ivec::to_real(i_comp) + fvec::to_real(f_comp)
}

/// Vectorized unprojection.
///
/// Writes reconstructed normalized (x,y,z) into out[0..2].
/// - out is intentionally "array-like" to match gl-matrix style.
pub fn unproject(out: &mut V3, i: IVec, f: FVec) -> &mut V3 {
    out[0] = unproject_comp(ivec::X(i), fvec::X(f));
    out[1] = unproject_comp(ivec::Y(i), fvec::Y(f));
    out[2] = unproject_comp(ivec::Z(i), fvec::Z(f));
    out
}

#[cfg(test)]
#[path = "projection.test.rs"]
mod test;
