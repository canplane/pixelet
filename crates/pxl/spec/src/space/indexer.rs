// pxl-spec/space/indexer.rs

//! A lattice address to its cell's index at a zoom no finer than the unit,
//! and back. Lanes are X 24 bits (wraps at the seam), Y 24, Z 16, each
//! signed, and each shifts on its own (`ivec::sh`): a whole-word shift would
//! leak bits across lanes. A lane narrower than a zoom's side collapses there.

use super::pvf3::ivec::{self, IVec};
use super::zoom::{shamt, Zoom};

/// A floor on each lane.
pub fn idx(zoom: Zoom, addr: IVec) -> IVec {
    ivec::sh(addr, -shamt(zoom))
}

/// `idx`'s inverse, to the cell's base.
pub fn from_idx(zoom: Zoom, idx: IVec) -> IVec {
    ivec::sh(idx, shamt(zoom))
}

#[cfg(test)]
#[path = "indexer.test.rs"]
mod test;
