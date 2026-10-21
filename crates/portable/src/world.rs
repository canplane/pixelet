// pxl-portable/world.rs

//! gpu-portable: the world's one query, what is in a node at its zoom, the
//! layers read in the order each is written over the one before
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): what is
//! placed over all, then the separation, the ways, over the ground and its
//! caves. Asked by every reader of the world, the GPU's cast and the CPU's
//! alike (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md
//! "The query between them"): each source answers from its own level of
//! detail, and what it holds stays behind it.

use crate::answer::{fills, slab_of, slab_word, In, Kind, ANY, PLACED_BLOCK};
use crate::area::{Area, Shard};
use crate::caves::Cave;
use crate::ground::{self, Held, VOID};
use crate::overwrite::{self, Cut, Lanes, BLOCK, SPLIT};
use crate::window::{Found, Squares};
use crate::{Zoom, LEAF_CELL};

/// What is in the node `c` at `zoom`, asked from `far` to `near`, its
/// caves, what is placed over it and its shard as its parent handed them,
/// and `place` where the ground's cells of a node over it were held, which
/// becomes its own (`ground::node`), with `trail` the ways' cursor in the
/// tree of the square `place` names (`ways::tree`, off `pool`): stepped
/// from its parent's where `place` names the same square, found from that
/// square's root where it names another, as `place` itself is found again
/// (`window::step`). A preview's `carve` and `hide` (`overwrite::Cut`) are
/// read with what is placed.
///
/// It is a block where `near` is its zoom or coarser, or past the ground's
/// cells where nothing placed or carved is finer in it (`overwrite::whole`):
/// the world holds nothing finer than the node there. A block is what is
/// placed in it, air included, else the ground's block with what the ways
/// write over it (their tree's leaf there, `overwrite::value`, a bore's air
/// the ground's void, `ways::tree::read`), one value whichever source holds it
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
pub fn node<H: Held, W: Squares + Area, L: Lanes, P: Lanes>(
    held: &mut H,
    squares: &W,
    lanes: &L,
    pool: &P,
    (cave, put, shard): (Cave, overwrite::Held, Shard),
    (place, trail): (&mut Found, &mut overwrite::Held),
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
            // The four looks a filling block is drawn by, a byte each
            // (`ground::word`): only the ground's looks, a placed cube of a
            // look, fit; one past a byte, an object's, is drawn by its value.
            let four = if fills(over.block) { overwrite::looks(lanes, put, hide, zoom, c, over.block) } else { [0; 4] };
            let bytes = four[0] < 256 && four[1] < 256 && four[2] < 256 && four[3] < 256;
            return (
                In {
                    kind: Kind::Block,
                    block: [PLACED_BLOCK | over.block, if bytes { ground::word(four) } else { 0 }],
                },
                block,
            );
        }
        if over.says == SPLIT {
            (placed, splits) = (overwrite::slab(lanes, put, zoom, c), true);
        }
    }
    let before = *place;
    let (i, cell, found, bound) = ground::node(held, cave, shard, place, (zoom, c), [far, near], block);
    if place.from() != before.from() || place.slot() != before.slot() || place.here() != before.here() {
        *trail = if place.here() {
            overwrite::cursor(pool, squares.root(place.from(), place.slot()), zoom, c)
        } else {
            overwrite::Held::NOTHING
        };
    }
    // The ways where the block's column was read: their leaf over it.
    let way = if block && i.kind == Kind::Block && found.here() {
        overwrite::value(pool, *trail, Cut::NONE, zoom, c)
    } else {
        VOID
    };
    let (value, wrote) = if way == VOID { (i.block, false) } else { (crate::ways::tree::read(way), true) };
    if block {
        // The ground's block's four, or a way's look four times where one
        // wrote it, whatever its value.
        let four = match (i.kind, wrote) {
            (Kind::Block, false) => ground::word(cell.four(zoom, i.block)),
            (Kind::Block, true) => ground::word([value; 4]),
            _ => 0,
        };
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
