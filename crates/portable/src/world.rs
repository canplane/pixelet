// pxl-portable/world.rs

//! gpu-portable: the world's one query, what is in a node at its zoom, the
//! layers read in the order each is written over the one before
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): what is
//! placed over all, then the separation, the ways, over the ground and its
//! caves. Asked by every reader of the world, the GPU's cast and the CPU's
//! alike (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md
//! "The query between them"): each source answers from its own level of
//! detail, and what it holds stays behind it.

use crate::area::{Area, Shard};
use crate::cast::{In, Kind};
use crate::caves::Cave;
use crate::ground::{self, Held, VOID};
use crate::overwrite::{self, Cut, Lanes, BLOCK, SPLIT};
use crate::scatter::PLACED_BLOCK;
use crate::window::{self, Found, Ran};
use crate::{Zoom, LEAF_CELL};

/// What is in the node `c` at `zoom`, asked from `far` to `near`, its
/// caves, what is placed over it and its shard as its parent handed them,
/// and `place` where the ground's cells of a node over it were held, which
/// becomes its own (`ground::node`). A preview's `carve` and `hide`
/// (`overwrite::Cut`) are read with what is placed.
///
/// It is a block where `near` is its zoom or coarser, or past the ground's
/// cells where nothing placed or carved is finer in it (`overwrite::whole`):
/// the world holds nothing finer than the node there. A block is what is
/// placed in it, air included, else the ground's block with what the ways
/// write over it (`window::ways_over`), one value whichever source holds it
/// (doc/minutes/20261005-the-ground-is-a-carto-palette.md, "One value
/// space"); `PLACED_BLOCK` over a placed value, the source control tells an
/// edit by, which nothing drawn reads. Its second word is the four looks
/// the ground's block is drawn by (`ground::Cell::four`, `ground::word`),
/// none for any other. Coarser, a split where
/// any layer may fill under it at a zoom asked, each by its own bound —
/// what is placed (`overwrite::over`), the ground's (`ground::slab`), the
/// ways' (`ways::slab`), each meeting the node (`ground::meets`) — its
/// block word where along z (`slab_word`);
/// else air; where the ground is not held, the lack. With it, whether it is
/// a block.
#[allow(clippy::too_many_arguments)]
pub fn node<H: Held, W: Ran + Area, L: Lanes>(
    held: &mut H,
    ways: &W,
    lanes: &L,
    (cave, put, shard): (Cave, overwrite::Held, Shard),
    place: &mut Found,
    (zoom, c): (Zoom, [i32; 3]),
    [far, near]: [Zoom; 2],
    [carve, hide]: [Cut; 2],
) -> (In<[u32; 2]>, bool) {
    let mut block = zoom >= near || zoom >= LEAF_CELL;
    // Where what is placed splits the node, where along z it may fill.
    let (mut placed, mut splits) = (ground::EMPTY, false);
    // Asked of what is placed only where something is, or a preview cuts.
    if put.holds() || carve.zoom != Cut::NONE.zoom {
        block = zoom >= near || zoom >= LEAF_CELL && overwrite::whole(put, carve, zoom, c);
        let over = overwrite::over(lanes, put, carve, hide, zoom, c, if block { zoom } else { near });
        if over.says == BLOCK {
            return (
                In {
                    kind: Kind::Block,
                    block: [PLACED_BLOCK | over.block, 0],
                },
                block,
            );
        }
        if over.says == SPLIT {
            (placed, splits) = (overwrite::slab(lanes, put, zoom, c), true);
        }
    }
    let (i, cell, found, bound) = ground::node(held, cave, shard, place, (zoom, c), [far, near], block);
    let value = if block && i.kind == Kind::Block {
        window::ways_over(ways, found, cell.runs, zoom, c[2], i.block)
    } else {
        i.block
    };
    if block {
        let four = if i.kind == Kind::Block { ground::word(cell.four(zoom, value)) } else { 0 };
        return (In { kind: i.kind, block: [value, four] }, block);
    }
    // Coarser, what is placed splits it where it may fill, and the ways
    // what the ground leaves air where they may; a split's slab is every
    // source's own joined: what is placed (`overwrite::slab`), the ground's
    // (`ground::slab`, unbounded where it lacks) and the ways' (`ways::slab`).
    let coarsest = if far > zoom + 1 { far } else { zoom + 1 };
    let ways = crate::ways::slab(bound, [coarsest, near]);
    let fill = ground::meets(ways, (zoom, c[2]), near);
    let kind = if splits || i.kind == Kind::Block && fill { Kind::Split } else { i.kind };
    if kind != Kind::Split {
        return (In { kind, block: [value, 0] }, block);
    }
    let own = match i.kind {
        Kind::Split => slab_of(i.block),
        Kind::Lacks => ANY,
        _ => ground::EMPTY,
    };
    let joined = ground::join(ground::join(own, ways), placed);
    (In { kind, block: [slab_word(joined), 0] }, block)
}

/// A split's block word (`node`): where anything may fill the node along z,
/// in leaf cells, `[lo, hi)` held as its lowest in the low half and its
/// highest, `hi − 1`, in the high, each an i16 held to i16's range — the
/// world's whole z, so i16's whole is anything along it (`ANY`), and
/// lowest over highest is nothing (`ground::EMPTY`), as a join keeps it. A
/// ray through the node outside it meets nothing there (pxl-portable cast).
/// Each source's own bound joined: the ground's (`ground::slab`), the
/// ways' (`ways::slab`), and what is placed (`overwrite::slab`), each off
/// what it holds, as each source answers.
pub fn slab_word([lo, hi]: [i32; 2]) -> u32 {
    let held = |v: i32| {
        if v < i16::MIN as i32 {
            i16::MIN as i32
        } else if v > i16::MAX as i32 {
            i16::MAX as i32
        } else {
            v
        }
    };
    ground::bound_word([held(lo), held(hi - 1)]) as u32
}

/// `slab_word`'s lowest and past its highest.
pub fn slab_of(w: u32) -> [i32; 2] {
    let [lo, last] = ground::bound_of(w as i32);
    [lo, last + 1]
}

/// Anything along z the world holds, as a slab: from its lowest leaf cell to
/// past its highest, half-open, i16's whole as `slab_word` holds it. Not a
/// relief of face heights, both held (`ground::UNBOUNDED`), which is another
/// fact in another unit.
pub const ANY: [i32; 2] = [i16::MIN as i32, i16::MAX as i32 + 1];
const _: () = assert!(LEAF_CELL == crate::UNIT_ZOOM, "a slab's leaf cell is the world's unit");
const _: () = assert!(
    -(1i32 << (crate::HEIGHT_BITS - 1)) == i16::MIN as i32 && (1i32 << (crate::HEIGHT_BITS - 1)) - 1 == i16::MAX as i32,
    "the world's z, in leaf cells, is i16's range"
);

/// Whether a block's value as `node` answers it fills its block: neither
/// void nor air, whichever source holds it. What is placed answers a voxel
/// alone (`overwrite::value`), so no decor or marker is among them.
pub fn fills(value: u32) -> bool {
    let v = value & !PLACED_BLOCK;
    v != VOID && v != crate::overwrite::AIR
}

/// Whether what is placed holds a block's value (`PLACED_BLOCK`): the
/// source control tells an edit and a strike by, which nothing drawn reads.
pub fn placed(value: u32) -> bool {
    value & PLACED_BLOCK != 0
}
