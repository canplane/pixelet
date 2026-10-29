// pxl-spec-core/world.rs

//! gpu-portable: the world's one query, what is in a node at its zoom, the
//! layers read in the order each is written over the one before
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): what is
//! placed over all, then the separation, the ways, over the ground and its
//! caves. Asked by every reader of the world, the GPU's cast and the CPU's
//! alike (doc/minutes/20261013-a-ray-asks-the-world-whether-a-node-is-empty.md
//! "The query between them"): each source answers from its own level of
//! detail, and what it holds stays behind it.

use crate::answer;
use crate::answer::{
    box_join, box_none, cover, eighths, fills, kept_word, In, Kind, BOX_NONE, BOX_WHOLE, FULL,
    PLACED_ASSET,
};
use crate::area::Shard;
use crate::cave::Cave;
use crate::ground::{self, Held, VOID};
use crate::overwrite::{self, Cut, Lanes, AIR, CELL, PLACED, SPLIT};
use crate::way;
use crate::way::tree;
use crate::window::{Bricks, Found};
use crate::{Zoom, LEAF_CELL, UNIT_ZOOM};

/// What a node is handed by the node over it, each source's own, so a
/// descent asks each node's sources once rather than again from the top:
/// its caves (`Cave`), what is placed (`overwrite::Held`, its cursor in the
/// objects' tree), what is known of the service area at its shard
/// (`area::Shard`), where the windows held the ground's cells of a node
/// over it (`window::Found`, `window::step`), and the ways' cursor in the
/// tree of the brick `ground` names (`way::tree`). The CPU's and a GPU's
/// alike.
#[derive(Clone, Copy)]
pub struct Handed {
    pub cave: Cave,
    pub placed: overwrite::Held,
    pub shard: Shard,
    pub ground: Found,
    pub ways: overwrite::Held,
}

/// A `Handed`'s words, as a GPU holds one in storage (`Handed::words`).
pub const HANDED_WORDS: u32 = 5;

impl Handed {
    /// Its words: its caves, what is placed, its shard, where its ground's
    /// cells were held, its ways' cursor.
    pub const fn words(&self) -> [u32; HANDED_WORDS as usize] {
        [
            self.cave.word(),
            self.placed.word(),
            self.shard.word(),
            self.ground.word(),
            self.ways.word(),
        ]
    }

    /// `words`'s `Handed`.
    pub const fn of_words(w: [u32; HANDED_WORDS as usize]) -> Handed {
        Handed {
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
    pub fn of(lanes: &impl Lanes, (zoom, c): (Zoom, [i32; 3])) -> Handed {
        Handed {
            cave: Cave::of(zoom, c),
            placed: overwrite::cursor(lanes, PLACED, zoom, c),
            shard: Shard::UNKNOWN,
            ground: Found::NONE,
            ways: overwrite::Held::NOTHING,
        }
    }

    /// `of` with nothing placed: the node as the world unplaced holds it,
    /// the ground's alone, which decor grows on (`scatter::Grounds::footing`);
    /// nothing placed stays nothing down a descent (`overwrite::child`).
    pub fn unplaced((zoom, c): (Zoom, [i32; 3])) -> At {
        At {
            cave: Cave::of(zoom, c),
            placed: overwrite::Held::NOTHING,
            shard: Shard::UNKNOWN,
            ground: Found::NONE,
            ways: overwrite::Held::NOTHING,
        }
    }

    /// What its child `c` at `zoom` is handed: each source stepped a zoom
    /// down, what is placed off `lanes` and the ways off `pool`.
    pub fn child(
        &self,
        lanes: &impl Lanes,
        pool: &impl Lanes,
        (zoom, c): (Zoom, [i32; 3]),
    ) -> Handed {
        Handed {
            cave: self.cave.child(zoom, c),
            placed: overwrite::child(lanes, self.placed, zoom, c),
            shard: self.shard,
            ground: self.ground,
            ways: overwrite::child(pool, self.ways, zoom, c),
        }
    }
}

/// What is in the node `c` at `zoom`, asked from `far` to `near`, `at` what
/// the node over it handed it (`Handed`): its shard is found first, before any
/// layer answers, so a stop's shard is its own whichever layer stops it;
/// where the ground's cells of a node over it were held becomes its own
/// (`ground::node`), and the ways' cursor is stepped from its parent's where
/// that names the same brick, found from that brick's root where it names
/// another, as the cells' place itself is found again (`window::step`). A
/// preview's `carve` and `hide` (`overwrite::Cut`) are read with what is
/// placed.
///
/// It is a cell where `near` is its zoom or coarser, or past the ground's
/// cells where nothing placed or carved is finer in it (`overwrite::whole`):
/// the world holds nothing finer than the node there. Its asset is what is
/// placed in it where that fills it whole, air included, else the ground's
/// asset with what the ways and what is placed write over it (their tree's
/// leaf or fold there, `overwrite::value`, a bore's air the ground's void,
/// `way::tree::read`), one value whichever source holds it, keeping the
/// eighths of its cube they fill together over it (`answer::cover`,
/// `answer::eighths`, `answer::KEPT_SHIFT`), air where that is none;
/// `PLACED_ASSET` over a placed value, the source control tells an edit by,
/// which nothing drawn reads. Its second word is the four looks the ground's
/// asset is drawn by (`ground::Cell::four`, `ground::word`), none for any
/// other. Coarser, a split where any layer may fill under it at a zoom asked,
/// each by its own bound — what is placed (`overwrite::over`), the ground's
/// (`ground::slab`), the ways' (`way::slab`), each meeting the node
/// (`ground::meets`) — its asset words the box in its own steps they lie in,
/// every axis alike (`answer::box_word`, `overwrite::bound`,
/// `ground::box_along_z`); else air, a cell of air its size, its value void;
/// where the ground is not held, the lack. Whether a cell is the zoom asked's
/// or coarser air is the asker's: its zoom against the finest asked (`near`),
/// past the ground's cells, where nothing finer is, a cell either way.
#[expect(
    clippy::too_many_arguments,
    reason = "the world's four stores, where the node is, and the zooms and cuts it is asked at, each its own"
)]
pub fn node<H: Held, W: Bricks, L: Lanes, P: Lanes>(
    held: &mut H,
    bricks: &W,
    lanes: &L,
    pool: &P,
    at: &mut Handed,
    (zoom, c): (Zoom, [i32; 3]),
    [far, near]: [Zoom; 2],
    [carve, hide]: [Cut; 2],
) -> In<[u32; 2]> {
    at.shard = at.shard.at(held, (zoom, [c[0], c[1]]));
    let (cave, put, shard) = (at.cave, at.placed, at.shard);
    let (place, trail) = (&mut at.ground, &mut at.ways);
    let is_cell = zoom >= near || zoom >= LEAF_CELL && overwrite::whole(put, carve, zoom, c);
    let over = overwrite::over(
        lanes,
        put,
        carve,
        hide,
        zoom,
        c,
        if is_cell { zoom } else { near },
    );
    // The four looks a filling asset of what is placed is drawn by, a byte
    // each (`ground::word`): only the ground's looks, a placed cube of a
    // look, fit; one past a byte, an object's, is drawn by its value.
    let laid = || {
        let four = if fills(over.asset) {
            overwrite::looks(lanes, put, hide, zoom, c, over.asset)
        } else {
            [0; 4]
        };
        let bytes = four[0] < 256 && four[1] < 256 && four[2] < 256 && four[3] < 256;
        [
            PLACED_ASSET | over.asset,
            if bytes { ground::word(four) } else { 0 },
        ]
    };
    // Filling its cube whole, what is placed is the cell's asset, whatever is
    // under it.
    if over.says == CELL && over.fill == FULL {
        return In {
            kind: Kind::Cell,
            asset: laid(),
        };
    }
    // Where what is placed splits the node, the box it may fill.
    let (placed, splits) = if over.says == SPLIT {
        (overwrite::bound(lanes, put, zoom), true)
    } else {
        (BOX_NONE, false)
    };
    let before = *place;
    let (i, cell, bound) = ground::node(held, cave, shard, place, (zoom, c), [far, near], is_cell);
    if place.from() != before.from()
        || place.slot() != before.slot()
        || place.here() != before.here()
    {
        *trail = if place.here() {
            tree::cursor(
                pool,
                bricks.root(place.from(), place.slot()),
                place.from(),
                zoom,
                c,
            )
        } else {
            overwrite::Held::NOTHING
        };
    }
    // The ways where the cell's column was read: their leaf or fold over it.
    let way = if is_cell && i.kind == Kind::Cell && place.here() {
        overwrite::val(pool, *trail, Cut::NONE, zoom, c)
    } else {
        VOID
    };
    if is_cell {
        // Where the ground is not held, the lack, unless what is placed
        // keeps an asset there alone.
        let lacks = In {
            kind: i.kind,
            asset: i.asset,
        };
        if i.kind == Kind::Lacks && over.says != CELL {
            return lacks;
        }
        // The ground's asset, the share of its cube it fills and its four
        // looks; then what each layer writes over it, the ways and what is
        // placed, its share kept with the ground's (`answer::cover`): the
        // cell keeps the eighths of its cube all of them fill
        // (`answer::eighths`), its look the kind most of that is.
        let mut val = if i.kind == Kind::Cell {
            i.asset[0]
        } else {
            VOID
        };
        let mut fill = if fills(val) { cell.cover(c[2]) } else { 0 };
        let mut four = if fills(val) {
            ground::word(cell.four(zoom, val))
        } else {
            0
        };
        if way != VOID {
            // A way's look four times where one wrote it, whatever its value.
            let w = tree::read(way);
            let (f, theirs) = cover(fill, w, overwrite::filled(pool, *trail));
            if theirs || !fills(val) {
                (val, four) = (w, ground::word([w; 4]));
            }
            fill = f;
        }
        if over.says == CELL {
            let (f, theirs) = cover(fill, over.asset, over.fill);
            if theirs || !fills(val) {
                [val, four] = laid();
            }
            fill = f;
        }
        let kept = eighths(fill);
        if kept == 0 && i.kind == Kind::Lacks {
            return lacks;
        }
        let asset = if kept == 0 {
            // Nothing kept: air, what is placed's where it is air.
            [
                if over.says == CELL && over.asset == AIR {
                    PLACED_ASSET | AIR
                } else {
                    VOID
                },
                0,
            ]
        } else {
            [kept_word(val, kept), four]
        };
        return In {
            kind: Kind::Cell,
            asset,
        };
    }
    // Coarser, what is placed splits it where it may fill, and the ways
    // what the ground leaves air where they may; a split's box is every
    // source's own joined, each in the node's steps: what is placed
    // (`overwrite::bound`), the ground's (`ground::slab`, all of it where
    // it lacks) and the ways' (`way::slab`), the last two along z.
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
    // Every source saying nothing lies in it is air its size.
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

/// The zoom a box's placed leaves are first asked at: 8 unit cells a side,
/// so a box with nothing placed near it is told so in a few looks.
const PLACED_FIRST: Zoom = UNIT_ZOOM - 3;
const PLACED_LEVELS: usize = (UNIT_ZOOM - PLACED_FIRST) as usize;

/// What is placed at the unit cells of a box, one leaf at a time
/// (`Placed::next`), each with its cell and its stored word, as the
/// objects' tree holds it (`overwrite`): asked of the query, never by
/// walking the tree beside it. A cursor rather than a callback, so a GPU,
/// which takes no closure over a binding's slice, walks it alike.
#[derive(Clone, Copy)]
pub struct Placed {
    lo: [i32; 3],
    hi: [i32; 3],
    /// The node at `PLACED_FIRST` asked next.
    first: [i32; 3],
    /// The nodes gone into, a level each, the child each is at, and the
    /// level at; none gone into at 0.
    held: [overwrite::Held; PLACED_LEVELS + 1],
    idx: [[i32; 3]; PLACED_LEVELS + 1],
    next: [u32; PLACED_LEVELS + 1],
    depth: usize,
}

/// The placed leaves at the unit cells of `[lo, hi]`, both ends held.
pub fn placed_in(lo: [i32; 3], hi: [i32; 3]) -> Placed {
    let s = UNIT_ZOOM - PLACED_FIRST;
    Placed {
        lo,
        hi,
        first: [lo[0] >> s, lo[1] >> s, lo[2] >> s],
        held: [overwrite::Held::NOTHING; PLACED_LEVELS + 1],
        idx: [[0; 3]; PLACED_LEVELS + 1],
        next: [0; PLACED_LEVELS + 1],
        depth: 0,
    }
}

impl Placed {
    /// The next leaf placed at a unit cell of the box, its cell and its
    /// word (`Lanes::data`); false past the last. One descent of the tree's
    /// branches meeting the box, each child stepped once from its parent.
    pub fn next(&mut self, lanes: &impl Lanes) -> ([i32; 3], u32, bool) {
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
                let node = overwrite::cursor(lanes, PLACED, PLACED_FIRST, n);
                if node.holds() {
                    (self.held[0], self.idx[0], self.next[1]) = (node, n, 0);
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
                overwrite::child(lanes, self.held[d - 1], z, c)
            } else {
                overwrite::Held::NOTHING
            };
            if h.holds() && h.is_folded() && d < PLACED_LEVELS {
                (self.held[d], self.idx[d], self.next[d + 1]) = (h, c, 0);
                self.depth += 1;
                continue;
            }
            self.next[d] += 1;
            if d == PLACED_LEVELS && h.holds() && !h.is_folded() && h.zoom() == UNIT_ZOOM {
                return (c, lanes.data(h.slot()), true);
            }
        }
    }
}

#[cfg(test)]
#[path = "world.test.rs"]
mod test;
