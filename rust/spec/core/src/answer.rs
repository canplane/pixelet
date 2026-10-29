// pxl-spec-core/answer.rs

//! gpu-portable: what the world's one query answers of a node
//! (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md,
//! "The query between them"), as every source writes it and every reader
//! reads it: its kind, a cell's asset in the one value space
//! (doc/minutes/20261005-the-ground-is-a-carto-palette.md) and the share of
//! its cube it keeps, and a split's box in its own steps, every axis alike.
//! Below the descent, the query and the sources alike.

use crate::cube::{object, sentinel};

/// A cube filled whole, as a share counts it: a share is in 2⁻¹⁵ of its
/// cube, floored at each fold, so eighths stay exact five folds deep.
pub const FULL: u32 = 1 << 15;

/// What a node is to a descent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    /// Coarser than asked: something under it may fill.
    Split,
    /// A cell: at the zoom asked, the asset in it; coarser, air (`AIR`)
    /// where nothing under it fills at the zooms asked, as a cell of air its
    /// size.
    Cell,
    /// Not held: past it the ray tells nothing.
    Lacks,
}

/// What is in a node: its kind, and its asset where it is a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct In<B> {
    pub kind: Kind,
    pub asset: B,
}

/// A cell nothing fills: void, no looks.
pub const AIR: [u32; 2] = [sentinel::void as u32, 0];

/// The bits an asset's value takes at the foot of its word.
pub const VAL_BITS: u32 = 16;

/// An asset's word where what is placed answered it, over its value: the
/// source control tells an edit by (`is_placed`), which nothing drawn reads.
/// Over it, the share of its cube the cell keeps (`KEPT_SHIFT`).
pub const PLACED_ASSET: u32 = 1 << VAL_BITS;

/// An asset's value, without whether what is placed holds it
/// (`PLACED_ASSET`) or what of its cube it keeps: a look reads the same
/// whichever layer holds it.
pub const fn val(asset: u32) -> u32 {
    asset & (PLACED_ASSET - 1)
}

/// A bound as a word, its lowest in the low half and its highest in the
/// high, each an i16: the ground's, a level's entry (`ground::Bounds`).
pub fn bound_word([lo, hi]: [i32; 2]) -> i32 {
    lo & 0xffff | hi << 16
}

/// `bound_word`'s lowest and highest.
pub fn bound_of(w: i32) -> [i32; 2] {
    [w << 16 >> 16, w >> 16]
}

/// A split's asset words: where anything may lie in the node, a box on
/// every axis alike in the node's own steps, `BOX_STEPS` a side, the
/// pyramid `BOX_BITS` zooms under it — on each axis its first step and its
/// last, `BOX_BITS` each: x's two and y's first in the first word, y's last
/// and z's two in the second. Conservative: what the node holds lies in it,
/// a box wider only splits more. A ray through the node outside it meets
/// nothing there (pxl-spec-core cast). The world asks no axis apart
/// (doc/minutes/archive/20260928-what-is-drawn-is-what-the-screen-covers.md,
/// "A box is all the view asks").
pub type BoxWords = [u32; 2];
pub const BOX_BITS: u32 = 10;
/// The steps of a box's axis: its node's side over them, `2^BOX_BITS`.
pub const BOX_STEPS: u32 = 1 << BOX_BITS;
pub const BOX_WHOLE: BoxWords = box_word([0; 3], [BOX_STEPS; 3]);
/// No box: nothing lies in the node, as a join of boxes starts. No
/// `box_word` sets a bit past `3 · BOX_BITS` of a word.
pub const BOX_NONE: BoxWords = [u32::MAX; 2];

/// The box from step `lo` to past step `hi` on each axis, each held to the
/// node: `lo` in `0..BOX_STEPS`, `hi` in `1..=BOX_STEPS`, at least one past
/// `lo`.
pub const fn box_word(lo: [u32; 3], hi: [u32; 3]) -> BoxWords {
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
pub const fn box_join(a: BoxWords, b: BoxWords) -> BoxWords {
    if a[0] == BOX_NONE[0] && a[1] == BOX_NONE[1] {
        return b;
    }
    if b[0] == BOX_NONE[0] && b[1] == BOX_NONE[1] {
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

/// Whether `b` is no box (`BOX_NONE`).
pub const fn box_none(b: BoxWords) -> bool {
    b[0] == BOX_NONE[0] && b[1] == BOX_NONE[1]
}

/// `box_word`'s first step and past its last on each axis.
pub const fn box_of(w: BoxWords) -> ([u32; 3], [u32; 3]) {
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

/// Whether an asset lets light through: a ray sees nothing there
/// and passes on, whichever source holds it. What every ray sees by
/// (cast.rs `walk`), and the one place a value is told to be so: what
/// nothing is painted on and what is placed as nothing. What is placed
/// answers a voxel alone (`overwrite::value`), so no decor or marker is
/// among them. A question of light, not of matter (`fills`): the two split
/// the values between them today only because no matter lets light
/// through yet, so neither is the other's negation; matter a ray sees
/// through would answer yes to both.
pub fn transparent(val: u32) -> bool {
    let v = self::val(val);
    v == sentinel::void as u32 || v == object::air as u32
}

/// Whether an asset is matter, taking its share of its cell as the
/// layers compose its volume (`cover`) and drawn by its looks: anything but
/// what nothing is painted on and what is placed as nothing. A question of
/// matter, not of light (`transparent`): matter a ray sees through would
/// still fill, so this is not `!transparent`.
pub fn fills(val: u32) -> bool {
    let v = self::val(val);
    v != sentinel::void as u32 && v != object::air as u32
}

/// A cell's kept share over its asset's value and `PLACED_ASSET`: the eighths of
/// its cube it keeps of what fills it (`kept`), 0 for all eight, so a leaf's
/// word, and every cell's whole, sets no bit of it.
pub const KEPT_SHIFT: u32 = VALUE_BITS + 1;
pub const KEPT_BITS: u32 = EIGHTHS.trailing_zeros();
/// A cube's eighths: its children, what a share is kept in.
pub const EIGHTHS: u32 = 8;

/// The eighths of a cube kept of `fill` of it, in `FULL`'s units:
/// floored, so a trace too small to fill one child keeps none and nothing is
/// drawn there. The one rule a cell's kept share is, the ground's top
/// (`ground::top`) and a cell of every layer together (`cover`) alike.
pub const fn eighths(fill: u32) -> u32 {
    let k = fill / (FULL / EIGHTHS);
    if k > EIGHTHS {
        EIGHTHS
    } else {
        k
    }
}

/// What of a cell's cube fills, and whether an overwrite's kind is its
/// look: the ground filling `ground` of it, under an overwrite — what is
/// placed, or the ways — of `kind` filling `over` of it, each in
/// `FULL`'s units. A kind that fills lies beside the ground, as
/// a thing stands on it, so their shares add; air lies in it, as a cut or a
/// bore, so it takes its share from the ground's. The look is the kind most
/// of the fill is: the overwrite's where it fills at least as much as the
/// ground, the ground's where air is what is written.
#[expect(
    clippy::implicit_saturating_sub,
    reason = "rust-gpu has no `saturating_sub`"
)]
pub fn cover(ground: u32, kind: u32, over: u32) -> (u32, bool) {
    if fills(kind) {
        let sum = ground + over;
        (if sum > FULL { FULL } else { sum }, over >= ground)
    } else {
        (if over > ground { 0 } else { ground - over }, false)
    }
}

/// `asset` keeping `k` eighths of its cube (`KEPT_SHIFT`).
pub const fn kept_word(asset: u32, k: u32) -> u32 {
    asset & !(((1 << KEPT_BITS) - 1) << KEPT_SHIFT) | (k % EIGHTHS) << KEPT_SHIFT
}

/// The eighths of its cube an asset's word keeps, all eight where it sets
/// none (`KEPT_SHIFT`).
pub const fn kept(asset: u32) -> u32 {
    let k = asset >> KEPT_SHIFT & ((1 << KEPT_BITS) - 1);
    if k == 0 {
        EIGHTHS
    } else {
        k
    }
}

/// The side of the box a cell keeping `k` eighths of its cube is drawn
/// as, a share of its cube's: the cube root of what it keeps, so its volume
/// is that share, inset about its cube's center on every axis alike. A
/// table, as a GPU has no cube root: `∛(k/8) = ∛k / 2`.
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

/// Which faces of a cell kept short of its cube touch a neighbour filled at
/// its height, a bit a face over `KEPT_SHIFT`'s: `2a` the low face on axis
/// `a`, `2a + 1` the high. Where its box is drawn follows from them alone,
/// so every reader draws and meets one shape (cast.rs `kept_bounds`): a
/// box spans an axis whose two faces touch, sits flush against the one
/// face that does, and is centered where none does, the axes it does not
/// span sharing its volume alike. A producer sets them only for what it
/// knows its neighbours by, the ground at the zoom its square holds; every
/// other cell's are clear, a box centered on every axis.
pub const TOUCH_SHIFT: u32 = KEPT_SHIFT + KEPT_BITS;
pub const TOUCH_BITS: u32 = 6;
const _: () = assert!(TOUCH_SHIFT + TOUCH_BITS <= 32);

/// The faces `asset`'s cell touches (`TOUCH_SHIFT`).
pub const fn touch(asset: u32) -> u32 {
    asset >> TOUCH_SHIFT & ((1 << TOUCH_BITS) - 1)
}

/// `asset` touching the faces `t` (`TOUCH_SHIFT`).
pub const fn touch_word(asset: u32, t: u32) -> u32 {
    asset & !(((1 << TOUCH_BITS) - 1) << TOUCH_SHIFT) | (t & ((1 << TOUCH_BITS) - 1)) << TOUCH_SHIFT
}

/// Whether an asset hides its cube whole: it is not transparent and keeps
/// all of it (`kept`). A cell beside a box closes in none of its edges.
pub fn whole(asset: u32) -> bool {
    !transparent(asset) && kept(asset) == EIGHTHS
}

/// Whether what is placed holds an asset's value (`PLACED_ASSET`).
pub fn is_placed(val: u32) -> bool {
    val & PLACED_ASSET != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A join holds both boxes; none joins to the other.
    #[test]
    fn join_holds_both() {
        let a = box_word([0, 0, 1], [400, 1024, 2]);
        let b = box_word([2, 4, 600], [1024, 6, 700]);
        assert_eq!(box_of(box_join(a, b)), ([0, 0, 1], [1024, 1024, 700]));
        assert_eq!(box_join(BOX_NONE, a), a);
        assert_eq!(box_of(BOX_WHOLE), ([0; 3], [BOX_STEPS; 3]));
    }
}
