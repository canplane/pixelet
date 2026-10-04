// pxl-spec/space/pvf3.rs

//! PVF3: an exact lattice point as packed words, a signed cell index
//! (`ivec`, 24/24/16 bits) and an unsigned fraction (`fvec`, 1/256 of a cell).
//! The lane widths are the stored format's.

pub mod core;
pub mod fvec;
pub mod ivec;
pub mod projection;
pub mod pvec;
#[allow(clippy::module_inception)]
pub mod pvf3;

pub use fvec::FVec;
pub use ivec::IVec;
pub use pvec::{Fvec, Ivec, PVec, PVecTraits};
pub use pvf3::Pvf3;

use self::core::{IVEC_Y_W, IVEC_Z_W};
use super::v3::Aabb;

/// Every point a position can name, in units. X is unbounded: the lane is one world round.
pub const REACH: Aabb = {
    let (y, z) = ((1u64 << (IVEC_Y_W - 1)) as f64, (1u64 << (IVEC_Z_W - 1)) as f64);
    Aabb {
        min: [f64::NEG_INFINITY, -y, -z],
        max: [f64::INFINITY, y, z],
    }
};
