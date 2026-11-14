// pxl-spec/share.rs

//! wgsl-port: how much of a cell's cube is filled, its share, as every layer keeps it and
//! every reader reads it. A share is counted in `FULL`'s units and kept in eighths of the
//! cube, its children.

/// A cube filled whole, as a share counts it. A share is counted in 2⁻¹⁵ of its cube and
/// floored at each fold, so its eighths stay exact through `FULL_FOLD_CNT` folds.
pub const FULL: u32 = 1 << (FILL_BITS * FULL_FOLD_CNT);

const FULL_FOLD_CNT: u32 = 5;

/// The bits a share's eighths take in a word.
pub const FILL_BITS: u32 = EIGHTH_CNT.trailing_zeros();
/// A cube's eighths, its children, in which a share is kept.
pub const EIGHTH_CNT: u32 = 8;

/// The eighths of a cube kept of `fill` of it, in `FULL`'s units. It floors, so a trace too
/// small to fill one child keeps none and nothing is drawn there. It is the one rule for a
/// cell's fill share, for the ground's top (ground.rs `top`) and for a cell of every layer
/// together (pxl-engine answer.rs `cover`) alike.
pub const fn eighths(fill: u32) -> u32 {
    let k = fill / (FULL / EIGHTH_CNT);
    if k > EIGHTH_CNT {
        EIGHTH_CNT
    } else {
        k
    }
}
