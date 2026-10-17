// pxl-portable/world.rs

//! gpu-portable: the world's one query, what is in a node at its zoom, the
//! layers read in the order each is written over the one before
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): what is
//! placed over all, then the separation, the ways, over the ground and its
//! caves. Asked by every reader of the world, the GPU's cast and the CPU's
//! alike (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md
//! "The query between them"): each source answers from its own level of
//! detail, and what it holds stays behind it.

use crate::area::Shard;
use crate::cast::{In, Kind};
use crate::caves::Cave;
use crate::ground::{self, Held, VOID};
use crate::overwrite::{self, Cut, Lanes, BLOCK, SPLIT};
use crate::scatter::PLACED_BLOCK;
use crate::window::{self, Found, Windows};
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
/// write over it (`window::ways_over`); `PLACED_BLOCK` over a placed value,
/// the ground's fold's four with it (`ground::word`). Coarser, a split where
/// any layer may fill under it at a zoom asked, else air; where the ground
/// is not held, the lack. With it, whether it is a block.
#[allow(clippy::too_many_arguments)]
pub fn node<H: Held, W: Windows, L: Lanes>(
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
            return (In { kind: Kind::Split, block: [VOID, 0] }, block);
        }
    }
    let (i, cell, found) = ground::node(held, cave, shard, place, (zoom, c), [far, near], block);
    let (value, f) = if block && i.kind == Kind::Block {
        window::ways_over(ways, found, cell.runs, zoom, c[2], i.block)
    } else {
        i.block
    };
    (
        In {
            kind: i.kind,
            block: [value, ground::word(f)],
        },
        block,
    )
}
