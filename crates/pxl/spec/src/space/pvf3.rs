// pxl-spec/space/pvf3.rs

//! PVF3: a point in the lattice as packed words, exact. A position is an
//! integer cell index and a fraction within the cell, and each is three
//! lanes in one word — `ivec` the signed index (24, 24 and 16 bits), `fvec`
//! the unsigned fraction (8 bits each, 1/256 of a cell) — so two positions
//! are equal when their words are, and a sum lands on a lane, never between
//! two doubles. `pvf3` is the pair as one fixed-point value per axis, and
//! `projection` is the way in from and out to doubles. The lanes are what
//! the pages store and the object store keys by, so their widths are the
//! format's.
//!
//! `pvec` is what the two lane words have in common, as a trait: the sparse
//! tree keys by whichever it is given.

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
