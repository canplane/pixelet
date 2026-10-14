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

/// The block `x, y, z` from a block, each `-1..=1`, as a bit of the 3 × 3 ×
/// 3 about it.
const fn about(x: i32, y: i32, z: i32) -> u32 {
    1 << ((z + 1) * 9 + (y + 1) * 3 + (x + 1))
}

/// The blocks about a block its face bits read (`mask`), as `about` names
/// them: the four beside it, level and a block over and under; the column
/// over and under it; the four corners, level and a block over.
pub const READS: u32 = {
    let (mut bits, mut z) = (0, -1);
    while z <= 1 {
        bits |= about(1, 0, z) | about(-1, 0, z) | about(0, 1, z) | about(0, -1, z);
        z += 1;
    }
    bits | about(0, 0, 1) | about(0, 0, -1) | about(1, 1, 0) | about(1, -1, 0) | about(-1, 1, 0) | about(-1, -1, 0) | about(1, 1, 1) | about(1, -1, 1) | about(-1, 1, 1) | about(-1, -1, 1)
};

/// Which of the blocks about a block fill, as `about` names them, `fills`
/// asked once of each `mask` reads (`READS`), from one place.
pub fn filled(mut fills: impl FnMut([i32; 3]) -> bool) -> u32 {
    let (mut bits, mut k) = (0, 0);
    while k < 27 {
        if READS >> k & 1 == 1 && fills([(k % 3) as i32 - 1, (k / 3 % 3) as i32 - 1, (k / 9) as i32 - 1]) {
            bits |= 1 << k;
        }
        k += 1;
    }
    bits
}

/// The face bits of a block, `filled` which of the blocks about it fill
/// (`filled`); the lateral kinds `+X, −X, +Y, −Y`, the corners `PP, PN, NP,
/// NN`, as their bits come.
pub fn mask(filled: u32) -> u32 {
    let f = |x: i32, y: i32, z: i32| filled & about(x, y, z) != 0;
    let mut m = ALL_FACES;
    let mut side = |k: u32, x: i32, y: i32| {
        if !f(x, y, 0) {
            m |= CREASE_BASE << k;
        }
        if f(x, y, 1) {
            m |= RISES_BASE << k;
        }
        if f(x, y, -1) {
            m |= DIPS_BASE << k;
        }
    };
    side(0, 1, 0);
    side(1, -1, 0);
    side(2, 0, 1);
    side(3, 0, -1);
    if f(0, 0, 1) {
        m |= CAP_PZ;
    }
    if f(0, 0, -1) {
        m |= CAP_NZ;
    }
    let mut corner = |k: u32, x: i32, y: i32| {
        if f(x, 0, 0) != f(0, y, 0) && f(x, y, 0) {
            m |= DIAG_PP << k;
        }
        if f(x, y, 1) {
            m |= DIAG_RISES_PP << k;
        }
    };
    corner(0, 1, 1);
    corner(1, 1, -1);
    corner(2, -1, 1);
    corner(3, -1, -1);
    m
}
