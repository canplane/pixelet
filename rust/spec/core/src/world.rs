// pxl-spec-core/world.rs

//! gpu-portable: the world's one query, what is in a node at its zoom, the
//! layers read in the order each is written over the one before
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): what is
//! placed over all, then the separation, the ways, over the ground and its
//! caves. Asked by every reader of the world, the GPU's cast and the CPU's
//! alike (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md
//! "The query between them"): each source answers from its own level of
//! detail, and what it holds stays behind it.

use crate::answer::{
    box_join, box_none, cover, eighths, fills, kept_word, In, Kind, BOX_NONE, BOX_WHOLE,
    PLACED_BLOCK,
};
use crate::area::Shard;
use crate::caves::Cave;
use crate::ground::{self, Held, VOID};
use crate::overwrite::{self, Cut, Lanes, AIR, BLOCK, FULL, PLACED, SPLIT};
use crate::window::{Found, Squares};
use crate::{Zoom, LEAF_CELL};

/// What a node is handed by the node over it, each source's own, so a
/// descent asks each node's sources once rather than again from the top:
/// its caves (`Cave`), what is placed (`overwrite::Held`, its cursor in the
/// objects' tree), what is known of the service area at its shard
/// (`area::Shard`), where the windows held the ground's cells of a node
/// over it (`window::Found`, `window::step`), and the ways' cursor in the
/// tree of the square `ground` names (`ways::tree`). The CPU's and a GPU's
/// alike.
#[derive(Clone, Copy)]
pub struct At {
    pub cave: Cave,
    pub placed: overwrite::Held,
    pub shard: Shard,
    pub ground: Found,
    pub ways: overwrite::Held,
}

/// An `At`'s words, as a GPU holds one in storage (`At::words`).
pub const AT_WORDS: u32 = 5;

impl At {
    /// Its words: its caves, what is placed, its shard, where its ground's
    /// cells were held, its ways' cursor.
    pub const fn words(&self) -> [u32; AT_WORDS as usize] {
        [
            self.cave.word(),
            self.placed.word(),
            self.shard.word(),
            self.ground.word(),
            self.ways.word(),
        ]
    }

    /// `words`'s `At`.
    pub const fn of_words(w: [u32; AT_WORDS as usize]) -> At {
        At {
            cave: Cave::of_word(w[0]),
            placed: overwrite::Held::of_word(w[1]),
            shard: Shard::of_word(w[2]),
            ground: Found::of_word(w[3]),
            ways: overwrite::Held::of_word(w[4]),
        }
    }

    /// The node `c` at `zoom` with nothing handed down, a descent's root
    /// among them: each source found down its line from the top, what is
    /// placed off `lanes`.
    pub fn of(lanes: &impl Lanes, (zoom, c): (Zoom, [i32; 3])) -> At {
        At {
            cave: Cave::of(zoom, c),
            placed: overwrite::cursor(lanes, PLACED, zoom, c),
            shard: Shard::UNKNOWN,
            ground: Found::NONE,
            ways: overwrite::Held::NOTHING,
        }
    }

    /// What its child `c` at `zoom` is handed: each source stepped a zoom
    /// down, what is placed off `lanes` and the ways off `pool`.
    pub fn child(&self, lanes: &impl Lanes, pool: &impl Lanes, (zoom, c): (Zoom, [i32; 3])) -> At {
        At {
            cave: self.cave.child(zoom, c),
            placed: overwrite::child(lanes, self.placed, zoom, c),
            shard: self.shard,
            ground: self.ground,
            ways: overwrite::child(pool, self.ways, zoom, c),
        }
    }
}

/// What is in the node `c` at `zoom`, asked from `far` to `near`, `at` what
/// the node over it handed it (`At`): its shard is found first, before any
/// layer answers, so a stop's shard is its own whichever layer stops it;
/// where the ground's cells of a node over it were held becomes its own
/// (`ground::node`), and the ways' cursor is stepped from its parent's where
/// that names the same square, found from that square's root where it names
/// another, as the cells' place itself is found again (`window::step`). A
/// preview's `carve` and `hide` (`overwrite::Cut`) are read with what is
/// placed.
///
/// It is a block where `near` is its zoom or coarser, or past the ground's
/// cells where nothing placed or carved is finer in it (`overwrite::whole`):
/// the world holds nothing finer than the node there. A block is what is
/// placed in it where that fills it whole, air included, else the ground's
/// block with what the ways and what is placed write over it (their tree's
/// leaf or fold there, `overwrite::value`, a bore's air the ground's void,
/// `ways::tree::read`), one value whichever source holds it
/// (doc/minutes/20261005-the-ground-is-a-carto-palette.md, "One value
/// space"), keeping the eighths of its cube they fill together over it
/// (`answer::cover`, `answer::eighths`, `answer::KEPT_SHIFT`), air where
/// that is none; `PLACED_BLOCK` over a placed value, the source control tells an
/// edit by, which nothing drawn reads. Its second word is the four looks
/// the ground's block is drawn by (`ground::Cell::four`, `ground::word`),
/// none for any other. Coarser, a split where
/// any layer may fill under it at a zoom asked, each by its own bound —
/// what is placed (`overwrite::over`), the ground's (`ground::slab`), the
/// ways' (`ways::slab`), each meeting the node (`ground::meets`) — its
/// block words the box in its own steps they lie in, every axis alike
/// (`answer::box_word`, `overwrite::bound`, `ground::box_along_z`);
/// else air, a block of air its size, its value void; where the ground is
/// not held, the lack. Whether a block is the zoom asked's or coarser air is
/// the asker's: its zoom against the finest asked (`near`), past the
/// ground's cells, where nothing finer is, a block either way.
#[allow(clippy::too_many_arguments)]
pub fn node<H: Held, W: Squares, L: Lanes, P: Lanes>(
    held: &mut H,
    squares: &W,
    lanes: &L,
    pool: &P,
    at: &mut At,
    (zoom, c): (Zoom, [i32; 3]),
    [far, near]: [Zoom; 2],
    [carve, hide]: [Cut; 2],
) -> In<[u32; 2]> {
    at.shard = at.shard.at(held, (zoom, [c[0], c[1]]));
    let (cave, put, shard) = (at.cave, at.placed, at.shard);
    let (place, trail) = (&mut at.ground, &mut at.ways);
    let block = zoom >= near || zoom >= LEAF_CELL && overwrite::whole(put, carve, zoom, c);
    let over = overwrite::over(
        lanes,
        put,
        carve,
        hide,
        zoom,
        c,
        if block { zoom } else { near },
    );
    // The four looks a filling block of what is placed is drawn by, a byte
    // each (`ground::word`): only the ground's looks, a placed cube of a
    // look, fit; one past a byte, an object's, is drawn by its value.
    let laid = || {
        let four = if fills(over.block) {
            overwrite::looks(lanes, put, hide, zoom, c, over.block)
        } else {
            [0; 4]
        };
        let bytes = four[0] < 256 && four[1] < 256 && four[2] < 256 && four[3] < 256;
        [
            PLACED_BLOCK | over.block,
            if bytes { ground::word(four) } else { 0 },
        ]
    };
    // Filling its cube whole, what is placed is the block, whatever is
    // under it.
    if over.says == BLOCK && over.fill == FULL {
        return In {
            kind: Kind::Block,
            block: laid(),
        };
    }
    // Where what is placed splits the node, the box it may fill.
    let (placed, splits) = if over.says == SPLIT {
        (overwrite::bound(lanes, put, zoom), true)
    } else {
        (BOX_NONE, false)
    };
    let before = *place;
    let (i, cell, bound) = ground::node(held, cave, shard, place, (zoom, c), [far, near], block);
    if place.from() != before.from()
        || place.berth() != before.berth()
        || place.here() != before.here()
    {
        *trail = if place.here() {
            overwrite::cursor(pool, squares.root(place.from(), place.berth()), zoom, c)
        } else {
            overwrite::Held::NOTHING
        };
    }
    // The ways where the block's column was read: their leaf or fold over it.
    let way = if block && i.kind == Kind::Block && place.here() {
        overwrite::value(pool, *trail, Cut::NONE, zoom, c)
    } else {
        VOID
    };
    if block {
        // Where the ground is not held, the lack, unless what is placed
        // keeps a block there alone.
        let lacks = In {
            kind: i.kind,
            block: i.block,
        };
        if i.kind == Kind::Lacks && over.says != BLOCK {
            return lacks;
        }
        // The ground's block, the share of its cube it fills and its four
        // looks; then what each layer writes over it, the ways and what is
        // placed, its share kept with the ground's (`answer::cover`): the
        // block keeps the eighths of its cube all of them fill
        // (`answer::eighths`), its look the kind most of that is.
        let mut value = if i.kind == Kind::Block {
            i.block[0]
        } else {
            VOID
        };
        let mut fill = if fills(value) { cell.cover(c[2]) } else { 0 };
        let mut four = if fills(value) {
            ground::word(cell.four(zoom, value))
        } else {
            0
        };
        if way != VOID {
            // A way's look four times where one wrote it, whatever its value.
            let w = crate::ways::tree::read(way);
            let (f, theirs) = cover(fill, w, overwrite::filled(pool, *trail));
            if theirs || !fills(value) {
                (value, four) = (w, ground::word([w; 4]));
            }
            fill = f;
        }
        if over.says == BLOCK {
            let (f, theirs) = cover(fill, over.block, over.fill);
            if theirs || !fills(value) {
                [value, four] = laid();
            }
            fill = f;
        }
        let kept = eighths(fill);
        if kept == 0 && i.kind == Kind::Lacks {
            return lacks;
        }
        let block = if kept == 0 {
            // Nothing kept: air, what is placed's where it is air.
            [
                if over.says == BLOCK && over.block == AIR {
                    PLACED_BLOCK | AIR
                } else {
                    VOID
                },
                0,
            ]
        } else {
            [kept_word(value, kept), four]
        };
        return In {
            kind: Kind::Block,
            block,
        };
    }
    // Coarser, what is placed splits it where it may fill, and the ways
    // what the ground leaves air where they may; a split's box is every
    // source's own joined, each in the node's steps: what is placed
    // (`overwrite::bound`), the ground's (`ground::slab`, all of it where
    // it lacks) and the ways' (`ways::slab`), the last two along z.
    let ways = crate::ways::slab(bound, [ground::coarsest(zoom, far), near]);
    let fill = ground::meets(ways, (zoom, c[2]), near);
    let kind = if splits || i.kind == Kind::Block && fill {
        Kind::Split
    } else {
        i.kind
    };
    if kind != Kind::Split {
        return In {
            kind,
            block: i.block,
        };
    }
    let own = match i.kind {
        Kind::Split => i.block,
        Kind::Lacks => BOX_WHOLE,
        _ => BOX_NONE,
    };
    let joined = box_join(
        box_join(own, ground::box_along_z(ways, (zoom, c[2]))),
        placed,
    );
    // Every source saying nothing lies in it is air its size.
    if box_none(joined) {
        return In {
            kind: Kind::Block,
            block: crate::answer::AIR,
        };
    }
    In {
        kind,
        block: joined,
    }
}
