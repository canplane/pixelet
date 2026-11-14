// pxl-engine/answer.rs

//! wgsl-port: the answer the world's one query gives about a node, as every source writes
//! it and every reader reads it: the node's kind; for a cell, its asset in the one value space
//! and the share of its cube it keeps; and for a split, the box where anything in it may lie,
//! in the node's own steps, alike on every axis. The descent, the query and the sources all
//! stand on this module.

use pxl_spec::asset::cube::{is_matter, Sentinel};
use pxl_spec::share::{EIGHTH_CNT, FILL_BITS, FULL};

/// What a node is to a descent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    /// Coarser than asked, and something under it may fill.
    Split,
    /// A cell. At the zoom asked it holds its asset. Coarser than asked, it is `EMPTY`, a void
    /// cell its whole size, where nothing under it fills at the zooms asked.
    Cell,
    /// Not held, a lack: past it the ray can tell nothing.
    Lack,
}

/// What is in a node: its kind, and its asset where it is a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct In<B> {
    pub kind: Kind,
    pub asset: B,
}

/// A cell nothing fills: void, no asset ids.
pub const EMPTY: [u32; 2] = [Sentinel::Void as u32, 0];

/// The low bits of an asset's word that hold its value.
pub const VAL_BITS: u32 = 16;

/// The bit above the value that marks an asset answered by what is placed. Control tells an
/// edit by it (`is_placed`); nothing drawn reads it. The share of its cube the cell keeps lies
/// above it (`FILL_SHIFT`).
pub const PLACED_ASSET: u32 = 1 << VAL_BITS;

/// An asset's value without the `PLACED_ASSET` bit or its fill share, so an asset id
/// reads the same whichever layer holds it.
pub const fn val(asset: u32) -> u32 {
    asset & (PLACED_ASSET - 1)
}

/// A bound packed into a word, its lowest in the low half and its highest in the high, each
/// as an i16. The ground's bounds, a clip level's entries, are kept so (`ground::Extent`).
pub(crate) fn bound_word([lo, hi]: [i32; 2]) -> i32 {
    lo & 0xffff | hi << 16
}

/// The lowest and highest a `bound_word` holds.
pub(crate) fn bound_of(w: i32) -> [i32; 2] {
    [w << 16 >> 16, w >> 16]
}

/// A split's asset words: the box in which anything in the node may lie, in the node's own
/// steps, `BOX_STEPS` a side on every axis, which is the pyramid `BOX_BITS` zooms under the
/// node. Each axis holds its first step and its last, `BOX_BITS` bits each: x's two and y's
/// first in the first word, y's last and z's two in the second. The box is conservative: all
/// the node holds lies in it, and a wider box only makes a ray split more. A ray through the
/// node outside the box meets nothing there (pxl-engine cast). The world asks no axis apart.
pub(crate) type PackedBox = [u32; 2];
pub const BOX_BITS: u32 = 10;
/// The steps on a box's axis; a step is the node's side divided by `2^BOX_BITS`.
pub const BOX_STEPS: u32 = 1 << BOX_BITS;
pub const BOX_WHOLE: PackedBox = box_word([0; 3], [BOX_STEPS; 3]);
/// No box: nothing lies in the node. A join of boxes starts from it. No `box_word` sets a bit
/// past `3 · BOX_BITS` of a word, so no box is mistaken for it.
pub const BOX_NONE: PackedBox = [u32::MAX; 2];

/// The box from step `lo` to past step `hi` on each axis, each held to the
/// node: `lo` in `0..BOX_STEPS`, `hi` in `1..=BOX_STEPS`, at least one past
/// `lo`.
pub(crate) const fn box_word(lo: [u32; 3], hi: [u32; 3]) -> PackedBox {
    let m = BOX_STEPS - 1;
    let mut f = [0u32; 6];
    let mut a = 0;
    while a < 3 {
        let l = if lo[a] > m { m } else { lo[a] };
        let h = if hi[a] <= l {
            l + 1
        } else if hi[a] > BOX_STEPS {
            BOX_STEPS
        } else {
            hi[a]
        };
        f[2 * a] = l;
        f[2 * a + 1] = h - 1;
        a += 1;
    }
    [
        f[0] | f[1] << BOX_BITS | f[2] << (2 * BOX_BITS),
        f[3] | f[4] << BOX_BITS | f[5] << (2 * BOX_BITS),
    ]
}

/// The least box holding boxes `a` and `b`, each a `box_word` or none.
pub(crate) const fn box_join(a: PackedBox, b: PackedBox) -> PackedBox {
    if box_none(a) {
        return b;
    }
    if box_none(b) {
        return a;
    }
    let (al, ah) = box_of(a);
    let (bl, bh) = box_of(b);
    let mut lo = [0; 3];
    let mut hi = [0; 3];
    let mut k = 0;
    while k < 3 {
        lo[k] = if al[k] < bl[k] { al[k] } else { bl[k] };
        hi[k] = if ah[k] > bh[k] { ah[k] } else { bh[k] };
        k += 1;
    }
    box_word(lo, hi)
}

pub(crate) const fn box_none(b: PackedBox) -> bool {
    b[0] == BOX_NONE[0] && b[1] == BOX_NONE[1]
}

/// The first step, and the step past the last, on each axis of a `box_word`.
pub(crate) const fn box_of(w: PackedBox) -> ([u32; 3], [u32; 3]) {
    let m = BOX_STEPS - 1;
    let f = [
        w[0] & m,
        w[0] >> BOX_BITS & m,
        w[0] >> (2 * BOX_BITS) & m,
        w[1] & m,
        w[1] >> BOX_BITS & m,
        w[1] >> (2 * BOX_BITS) & m,
    ];
    ([f[0], f[2], f[4]], [f[1] + 1, f[3] + 1, f[5] + 1])
}

/// Whether an asset is matter (pxl-spec `cube::is_matter`), whichever source holds it: it
/// takes its share of its cell as the layers compose the cell's volume (`cover`), it is drawn
/// by its asset ids, and a ray stops at it; a ray sees through anything else and passes
/// on (cast.rs `walk`).
pub fn fills(asset: u32) -> bool {
    is_matter(val(asset))
}

/// A cell's fill share, above its asset's value and `PLACED_ASSET`: the eighths of its cube
/// that what fills it keeps (`fill_eighths`), with 0 for all eight, so a leaf's word, like
/// every whole cell's, sets none of its bits.
pub const FILL_SHIFT: u32 = VAL_BITS + 1;

/// How much of a cell's cube fills, and whether the overwrite's kind gives the asset id.
/// The ground fills `ground` of the cube, and over it an overwrite (what is placed, or the
/// ways) of `kind` fills `over`, each in `FULL`'s units. A kind that fills lies beside the
/// ground, as a thing stands on it, so the two shares add. Air lies inside the ground, as a
/// cut, a bore or a cave does, so it takes its share from the ground's, in whole eighths as a
/// fill is kept (`eighths`): a trace too small to empty one child empties none. The asset id
/// is the kind that makes up most of the fill: the overwrite's where it fills at least
/// as much as the ground, and the ground's where air is written.
pub fn cover(ground: u32, kind: u32, over: u32) -> (u32, bool) {
    if fills(kind) {
        let sum = ground + over;
        (if sum > FULL { FULL } else { sum }, over >= ground)
    } else {
        let over = over / (FULL / EIGHTH_CNT) * (FULL / EIGHTH_CNT);
        (ground.saturating_sub(over), false)
    }
}

/// `asset` set to keep `k` eighths of its cube (`FILL_SHIFT`).
pub(crate) const fn fill_word(asset: u32, k: u32) -> u32 {
    asset & !(((1 << FILL_BITS) - 1) << FILL_SHIFT) | (k % EIGHTH_CNT) << FILL_SHIFT
}

/// The eighths of its cube an asset's word keeps; all eight where it sets none
/// (`FILL_SHIFT`).
pub const fn fill_eighths(asset: u32) -> u32 {
    let k = asset >> FILL_SHIFT & ((1 << FILL_BITS) - 1);
    if k == 0 {
        EIGHTH_CNT
    } else {
        k
    }
}

/// The side of the box a cell keeping `k` eighths of its cube is drawn as, as a share of the
/// cube's side: the cube root of what it keeps, so the box's volume is that share, inset about
/// the cube's center alike on every axis. It is a table because a GPU has no cube root:
/// `∛(k/8) = ∛k / 2`.
pub const fn side(k: u32) -> f32 {
    match k {
        1 => 0.5,
        2 => 0.629_960_5,
        3 => 0.721_124_8,
        4 => 0.793_700_5,
        5 => 0.854_988,
        6 => 0.908_560_3,
        7 => 0.956_465_6,
        _ => 1.0,
    }
}

/// Whether an asset hides its cube whole: it fills and keeps all of the cube
/// (`fill_eighths`). A box filled short of its cube closes in none of the edges of a cell
/// beside it.
pub fn whole(asset: u32) -> bool {
    fills(asset) && fill_eighths(asset) == EIGHTH_CNT
}

/// Whether what is placed holds an asset's value (`PLACED_ASSET`).
pub fn is_placed(asset: u32) -> bool {
    asset & PLACED_ASSET != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use pxl_spec::share::eighths;

    /// A join holds both boxes; none joins to the other.
    #[test]
    fn join_holds_both() {
        let a = box_word([0, 0, 1], [400, 1024, 2]);
        let b = box_word([2, 4, 600], [1024, 6, 700]);
        assert_eq!(box_of(box_join(a, b)), ([0, 0, 1], [1024, 1024, 700]));
        assert_eq!(box_join(BOX_NONE, a), a);
        assert_eq!(box_of(BOX_WHOLE), ([0; 3], [BOX_STEPS; 3]));
    }

    /// A carve takes a fill in whole eighths: a trace under one eighth takes none, as a fill
    /// under one keeps none (`eighths`).
    #[test]
    fn carve_takes_whole_eighths() {
        let air = Sentinel::Air as u32;
        let eighth = FULL / EIGHTH_CNT;
        assert_eq!(cover(FULL, air, eighth - 1), (FULL, false));
        assert_eq!(cover(FULL, air, eighth + 5), (FULL - eighth, false));
        assert_eq!(cover(FULL, air, 3 * eighth), (FULL - 3 * eighth, false));
        assert_eq!(cover(eighth, air, FULL), (0, false));
        assert_eq!(eighths(cover(FULL, air, 300).0), EIGHTH_CNT);
    }
}
