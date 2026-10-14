// pxl-portable/mask.rs

//! gpu-portable: what a block's face shows and is shaded by, read off the
//! blocks about it, the one rule a chunk's box and a ray's hit both take:
//! every face; along each edge of its top and bottom, and each upright edge,
//! a crease where the block across it does not fill, and where the one over
//! it rises or the one under it dips; over and under it, a cap where its own
//! column goes on; at each upright edge, the block at the corner where only
//! one of the two faces it joins is open; over its corners, the blocks that
//! rise there.

macro_rules! bits {
    ($($name:ident = $v:expr),* $(,)?) => {
        $(pub const $name: u32 = $v;)*
        /// The mask is a word, so bits run to 31.
        const _: () = assert!($($v < 1u64 << 32)&&*, "a mask bit past the word");
    };
}
include!("mask.bits.rs");

pub const ALL_FACES: u32 = FACE_BASE * ((1 << 6) - 1);
/// Every lateral edge a crease.
pub const ALL_CREASES: u32 = CREASE_BASE * 0xf;

/// The four lateral neighbours, `+X, −X, +Y, −Y`, as a lateral kind's bits
/// come.
const SIDES: [[i32; 2]; 4] = [[1, 0], [-1, 0], [0, 1], [0, -1]];
/// The four corners, `PP, PN, NP, NN`, as the diagonal bits come.
const CORNERS: [[i32; 2]; 4] = [[1, 1], [1, -1], [-1, 1], [-1, -1]];

/// The face bits of a block, `fills` asked of the block that far from it.
pub fn mask(fills: impl Fn([i32; 3]) -> bool) -> u32 {
    let mut m = ALL_FACES;
    let mut k = 0;
    while k < 4 {
        let [x, y] = SIDES[k];
        if !fills([x, y, 0]) {
            m |= CREASE_BASE << k;
        }
        if fills([x, y, 1]) {
            m |= RISES_BASE << k;
        }
        if fills([x, y, -1]) {
            m |= DIPS_BASE << k;
        }
        k += 1;
    }
    if fills([0, 0, 1]) {
        m |= CAP_PZ;
    }
    if fills([0, 0, -1]) {
        m |= CAP_NZ;
    }
    let mut k = 0;
    while k < 4 {
        let [x, y] = CORNERS[k];
        if fills([x, 0, 0]) != fills([0, y, 0]) && fills([x, y, 0]) {
            m |= DIAG_PP << k;
        }
        if fills([x, y, 1]) {
            m |= DIAG_RISES_PP << k;
        }
        k += 1;
    }
    m
}
