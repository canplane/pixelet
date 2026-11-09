// pxl-engine/world.rs

//! gpu-portable: the world's one query, which says what is in a node at its zoom. It reads
//! the layers in the order each is written over the one before: what is placed over all,
//! then the separation and the ways, over the ground and its caves. Every reader of the world
//! asks it, the GPU's cast and the CPU's alike. Each source answers from its own level of
//! detail, and what a source holds stays behind it.

use crate::answer;
use crate::answer::{
    box_join, box_none, cover, eighths, fill_word, fills, In, Kind, BOX_NONE, BOX_WHOLE, FULL,
    PLACED_ASSET,
};
use crate::area::Shard;
use crate::clipmap::{self, BrickSource, Lookup, Pool};
use crate::ground::{self, VOID};
use crate::object::{self, Cut, Table, AIR, CELL, ROOT, SPLIT};
use crate::way;
use crate::{Zoom, LEAF_CELL_ZOOM, UNIT_ZOOM};

/// What a node is handed by the node over it, each source's own part, so that a descent asks
/// each node's sources once. It holds what is placed (`object::NodeSlot`, a cursor in the
/// objects' tree), what is known of the service area at the node's shard (`area::Shard`),
/// and where the clip levels held the ground's cells of a node above it (`clipmap::Lookup`,
/// `clipmap::step`). The CPU and the GPU hand down the same. The caves are not part of it: a
/// cube's caves are its own (`Content::hollow`).
#[derive(Clone, Copy)]
pub struct Inheritance {
    pub object: object::NodeSlot,
    pub shard: Shard,
    pub ground: Lookup,
}

/// An `Inheritance`'s words, as a GPU holds one in storage (`Inheritance::words`).
pub(crate) const INHERITANCE_WORDS: u32 = 3;

impl Inheritance {
    pub const fn words(&self) -> [u32; INHERITANCE_WORDS as usize] {
        [self.object.word(), self.shard.word(), self.ground.word()]
    }

    pub const fn of_words(w: [u32; INHERITANCE_WORDS as usize]) -> Inheritance {
        Inheritance {
            object: object::NodeSlot::of_word(w[0]),
            shard: Shard::of_word(w[1]),
            ground: Lookup::of_word(w[2]),
        }
    }

    /// The inheritance of the node `c` at `zoom` with nothing handed down, as for a descent's
    /// root: each source is found down its line from the top, what is placed from `columns`.
    pub fn of(columns: &impl Table, (zoom, c): (Zoom, [i32; 3])) -> Inheritance {
        Inheritance {
            object: object::cursor(columns, ROOT, zoom, c),
            shard: Shard::UNKNOWN,
            ground: Lookup::NONE,
        }
    }

    /// `of` with nothing placed, for any node: the node as the world holds it with nothing
    /// placed, the ground's alone, on which a content's decor generates. Nothing placed stays
    /// nothing down a descent (`object::child`).
    pub const UNPLACED: Inheritance = Inheritance {
        object: object::NodeSlot::NOTHING,
        shard: Shard::UNKNOWN,
        ground: Lookup::NONE,
    };

    /// What its child `c` at `zoom` is handed: each source stepped one zoom down, what is placed
    /// in `columns`.
    pub fn child(&self, columns: &impl Table, (zoom, c): (Zoom, [i32; 3])) -> Inheritance {
        Inheritance {
            object: object::child(columns, self.object, zoom, object::path(zoom, c)),
            shard: self.shard,
            ground: self.ground,
        }
    }
}

/// What is in the node `c` at `zoom`, asked from `far` to `near`, given what the node above
/// handed it (`at`). Its shard is found first, before any layer answers, so a stop's shard is
/// its own whichever layer stops it. Where the ground's cells of a node above were held
/// becomes the node's own (`ground::node`). A preview's `carve` and `hide` (`object::Cut`) are
/// read with what is placed.
///
/// The node is a cell where `near` is its zoom or coarser, or where it lies past the ground's
/// cells and nothing placed or carved in it is finer (`object::whole`): the world holds
/// nothing finer than the node there. A cell's asset is what is placed in it where that fills
/// it whole, air included. Otherwise it is the ground's asset with what the ways write over it
/// (the rule over the spans the brick's fill kept, `clipmap::way_at`, with a bore's air as the
/// ground's void) and what is placed (its leaf or fold there, `object::val`): one value
/// whichever source holds it. The cell keeps
/// the eighths of its cube that they fill together (`answer::cover`, `answer::eighths`,
/// `answer::FILL_SHIFT`), and is air where that is none. `PLACED_ASSET` is set over a placed
/// value; control tells an edit by it, and nothing drawn reads it. The second word holds the
/// four palette indices the ground's asset is drawn by (`ground::Cell::four`,
/// `ground::word`), and is zero for any other asset.
///
/// A coarser node is a split where any layer may fill under it at a zoom asked, each layer
/// judged by its own bound: what is placed (`object::over`), the ground's (`ground::slab`) and
/// the ways' (`way::slab`), each tested against the node (`ground::meets`). A split's asset
/// words are the box, in the node's own steps and alike on every axis, in which those layers
/// lie (`answer::box_word`, `object::object_box`, `ground::box_along_z`). Otherwise the node
/// is air, a cell of air its size with the value void, and where the ground is not held it is
/// the lack. Whether a cell is at the zoom asked or is coarser air is for the asker to tell,
/// by its zoom against the finest asked (`near`); past the ground's cells, where nothing finer
/// exists, it is a cell either way.
#[expect(
    clippy::too_many_arguments,
    reason = "the world's four stores, where the node is, and the zooms and cuts it is asked at, each its own"
)]
pub fn node<H: ground::Resident, W: BrickSource, L: Table, P: Pool + ?Sized>(
    resident: &mut H,
    bricks: &W,
    columns: &L,
    pool: &P,
    at: &mut Inheritance,
    (zoom, c): (Zoom, [i32; 3]),
    [far, near]: [Zoom; 2],
    [carve, hide]: [Cut; 2],
) -> In<[u32; 2]> {
    at.shard = at.shard.at(resident, (zoom, [c[0], c[1]]));
    let (put, shard) = (at.object, at.shard);
    let place = &mut at.ground;
    let is_cell = zoom >= near || zoom >= LEAF_CELL_ZOOM && object::whole(put, carve, zoom, c);
    let over = object::over(
        columns,
        put,
        carve,
        hide,
        zoom,
        c,
        if is_cell { zoom } else { near },
    );
    // Where what is placed is a cell: its asset, and the four palette indices a filling one is
    // drawn by, a byte each (`ground::word`). Only the ground's palette indices, as in a placed
    // cube of a palette index, fit in a byte; an object's value lies past a byte and is drawn by
    // its value. It is read once, here, for both uses below.
    let laid = if over.answer == CELL {
        let four = if fills(over.asset) {
            object::palette_idxs(columns, put, hide, zoom, c, over.asset)
        } else {
            [0; 4]
        };
        let bytes = four[0] < 256 && four[1] < 256 && four[2] < 256 && four[3] < 256;
        [
            PLACED_ASSET | over.asset,
            if bytes { ground::word(four) } else { 0 },
        ]
    } else {
        [0; 2]
    };
    // Where what is placed fills its cube whole, it is the cell's asset, whatever lies under it.
    if over.answer == CELL && over.fill == FULL {
        return In {
            kind: Kind::Cell,
            asset: laid,
        };
    }
    // Where what is placed splits the node, the box it may fill.
    let (placed, splits) = if over.answer == SPLIT {
        (object::object_box(columns, put, zoom), true)
    } else {
        (BOX_NONE, false)
    };
    let (i, cell, bound) = ground::node(resident, shard, place, (zoom, c), [far, near], is_cell);
    // The ways' cell over the ground's, where the cell's column was read.
    let way = if is_cell && i.kind == Kind::Cell && place.here() {
        clipmap::way_at(bricks, pool, *place, bound, (zoom, c))
    } else {
        way::NOTHING
    };
    if is_cell {
        // Where the ground is not held, the cell is the lack, unless what is placed keeps an asset
        // there alone.
        let lacks = In {
            kind: i.kind,
            asset: i.asset,
        };
        if i.kind == Kind::Lacks && over.answer != CELL {
            return lacks;
        }
        // Start from the ground's asset, the share of its cube it fills and its four palette
        // indices. Then each layer that writes over it, the ways and then what is placed, adds its
        // share to the ground's (`answer::cover`). The cell keeps the eighths of its cube that all of
        // them fill (`answer::eighths`), and its palette index is the kind that makes up most of that.
        let mut val = if i.kind == Kind::Cell {
            i.asset[0]
        } else {
            VOID
        };
        // The ground's share, less what its caves take (the second word of the ground's answer,
        // `ground::node`).
        let mut fill = if fills(val) {
            let (f, _) = cover(cell.cover(c[2]), AIR, i.asset[1]);
            f
        } else {
            0
        };
        let mut four = if fills(val) {
            ground::word(cell.four(zoom, val))
        } else {
            0
        };
        if way != way::NOTHING {
            // Where a way wrote the cell, its palette index four times, whatever its value.
            let (f, theirs) = cover(fill, way, FULL);
            if theirs {
                (val, four) = (way, ground::word([way; 4]));
            }
            fill = f;
        }
        if over.answer == CELL {
            let (f, theirs) = cover(fill, over.asset, over.fill);
            if theirs {
                [val, four] = laid;
            }
            fill = f;
        }
        let filled = eighths(fill);
        if filled == 0 && i.kind == Kind::Lacks {
            return lacks;
        }
        let asset = if filled == 0 {
            // Nothing is kept: the cell is air, and placed air where what is placed is air.
            [
                if over.answer == CELL && over.asset == AIR {
                    PLACED_ASSET | AIR
                } else {
                    VOID
                },
                0,
            ]
        } else {
            [fill_word(val, filled), four]
        };
        return In {
            kind: Kind::Cell,
            asset,
        };
    }
    // Coarser: what is placed splits the node where it may fill, and the ways split what the
    // ground leaves air where they may fill. A split's box joins every source's own box, each in
    // the node's steps: what is placed (`object::object_box`), the ground's (`ground::slab`, all
    // of the node where the ground lacks) and the ways' (`way::slab`), the last two along z.
    let ways = way::slab(bound, [ground::coarsest(zoom, far), near]);
    let fill = ground::meets(ways, (zoom, c[2]), near);
    let kind = if splits || i.kind == Kind::Cell && fill {
        Kind::Split
    } else {
        i.kind
    };
    if kind != Kind::Split {
        return In {
            kind,
            asset: i.asset,
        };
    }
    let own = match i.kind {
        Kind::Split => i.asset,
        Kind::Lacks => BOX_WHOLE,
        _ => BOX_NONE,
    };
    let joined = box_join(
        box_join(own, ground::box_along_z(ways, (zoom, c[2]))),
        placed,
    );
    // Where every source says nothing lies in it, the node is air its size.
    if box_none(joined) {
        return In {
            kind: Kind::Cell,
            asset: answer::AIR,
        };
    }
    In {
        kind,
        asset: joined,
    }
}

/// The zoom at which a box's placed leaves are first asked: nodes 8 unit cells a side, so a
/// box with nothing placed near it is told so after a few nodes.
pub const PLACED_FIRST: Zoom = UNIT_ZOOM - 3;
const PLACED_LEVELS: usize = (UNIT_ZOOM - PLACED_FIRST) as usize;

/// What is placed at the unit cells of a box, one leaf at a time (`PlacedCursor::next`), each
/// with its cell and its stored word as the objects' tree holds it (`object`). It is asked of
/// the query. It is a cursor rather than a callback so that a GPU, which takes no closure over
/// a binding's slice, walks it the same way.
#[derive(Clone, Copy)]
pub struct PlacedCursor {
    lo: [i32; 3],
    hi: [i32; 3],
    /// The node at `PLACED_FIRST` asked next.
    first: [i32; 3],
    /// The nodes gone into, one a level, with the child each is at and the current level
    /// (`depth`); at depth 0 none is gone into.
    nodes: [object::NodeSlot; PLACED_LEVELS + 1],
    idx: [[i32; 3]; PLACED_LEVELS + 1],
    next: [u32; PLACED_LEVELS + 1],
    depth: usize,
}

/// The placed leaves at the unit cells of `[lo, hi]`, both ends included.
pub fn placed_in(lo: [i32; 3], hi: [i32; 3]) -> PlacedCursor {
    let s = UNIT_ZOOM - PLACED_FIRST;
    PlacedCursor {
        lo,
        hi,
        first: [lo[0] >> s, lo[1] >> s, lo[2] >> s],
        nodes: [object::NodeSlot::NOTHING; PLACED_LEVELS + 1],
        idx: [[0; 3]; PLACED_LEVELS + 1],
        next: [0; PLACED_LEVELS + 1],
        depth: 0,
    }
}

impl PlacedCursor {
    /// The next leaf placed at a unit cell of the box, with its cell and its word
    /// (`Table::data`), or false past the last. The cursor makes one descent of the tree's
    /// branches that meet the box, stepping each child once from its parent.
    pub fn next(&mut self, columns: &impl Table) -> ([i32; 3], u32, bool) {
        let s = UNIT_ZOOM - PLACED_FIRST;
        loop {
            if self.depth == 0 {
                if self.first[2] > self.hi[2] >> s {
                    return ([0; 3], 0, false);
                }
                let n = self.first;
                self.first[0] += 1;
                if self.first[0] > self.hi[0] >> s {
                    self.first[0] = self.lo[0] >> s;
                    self.first[1] += 1;
                    if self.first[1] > self.hi[1] >> s {
                        self.first[1] = self.lo[1] >> s;
                        self.first[2] += 1;
                    }
                }
                let node = object::cursor(columns, ROOT, PLACED_FIRST, n);
                if node.holds() {
                    (self.nodes[0], self.idx[0], self.next[1]) = (node, n, 0);
                    self.depth = 1;
                }
                continue;
            }
            let d = self.depth;
            if self.next[d] == 8 {
                self.depth -= 1;
                self.next[self.depth] += 1;
                continue;
            }
            let (o, p) = (self.next[d], self.idx[d - 1]);
            let c = [
                2 * p[0] + (o & 1) as i32,
                2 * p[1] + (o >> 1 & 1) as i32,
                2 * p[2] + (o >> 2 & 1) as i32,
            ];
            let z = PLACED_FIRST + d as Zoom;
            let t = UNIT_ZOOM - z;
            let meets = c[0] << t <= self.hi[0]
                && (c[0] + 1) << t > self.lo[0]
                && c[1] << t <= self.hi[1]
                && (c[1] + 1) << t > self.lo[1]
                && c[2] << t <= self.hi[2]
                && (c[2] + 1) << t > self.lo[2];
            let h = if meets {
                object::child(columns, self.nodes[d - 1], z, c)
            } else {
                object::NodeSlot::NOTHING
            };
            if h.holds() && h.is_folded() && d < PLACED_LEVELS {
                (self.nodes[d], self.idx[d], self.next[d + 1]) = (h, c, 0);
                self.depth += 1;
                continue;
            }
            self.next[d] += 1;
            if d == PLACED_LEVELS && h.holds() && !h.is_folded() && h.zoom() == UNIT_ZOOM {
                return (c, columns.data(h.slot()), true);
            }
        }
    }
}

#[cfg(test)]
#[path = "world.test.rs"]
mod test;
