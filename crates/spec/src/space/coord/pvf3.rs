// pxl-spec/space/coord/pvf3.rs

//! PVF3, the Packed 3-Vector with Fixed-point arithmetic: an exact lattice
//! point as packed words, a signed cell index (`ivec`, 24/24/16 bits) and an
//! unsigned fraction (`fvec`, 1/256 of a cell), the same bits on every
//! machine. It is a format and its arithmetic, not a world: the lane widths
//! are the stored format's, and what a lane means — X going round, Y the
//! Mercator square's — is the world's (`space/coord/world.rs`).

pub mod core;
pub mod fvec;
pub mod ivec;
pub mod pack;
pub mod pvec;
#[allow(clippy::module_inception)]
pub mod pvf3;

pub use fvec::FVec;
pub use ivec::IVec;
pub use pvec::{Fvec, Ivec, PVec, PVecTraits};
pub use pvf3::Pvf3;
