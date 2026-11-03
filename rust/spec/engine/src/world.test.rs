// pxl-engine/world.test.rs

extern crate std;

use super::*;
use crate::area::{self, Area, Shard};
use crate::cast::{End, Query};
use crate::clipmap::{
    self, bound_over, column_top, entry, fold, join, joined_idx, level_idx, of_idx, Entry,
    Presence, ACROSS, BRICK, BRICK_CELLS, BRICK_SHIFT, BRICK_WORDS, CLIP_LEVELS, CLIP_SIZE,
    COLUMNS_AT, ENTRIES, EYE_CLIP_LEVELS, FIRST_CELL, JOINED_ENTRIES, JOINED_LEVELS, JOINED_WORDS,
    LAID_AT, LAID_CELLS, LEVELS, LEVELS_AT, LEVEL_ENTRY_WORDS, ORIGIN_AT,
};
use crate::content::Sample;
use crate::fill::{column_of, laid_of, level_of, put, root_of, LoneBrick};
use crate::ground::Bounds;
use crate::support::{asset, column_surface};
use crate::SHARD_ZOOM;
use std::{vec, vec::Vec};

/// The clip level whose bricks are half held, its east half not yet, the clip
/// levels over it held whole: the leaves', as decor's column surfaces ask,
/// where a node wider than a brick may stand clear over the ground.
const CELL: Zoom = LEAF_CELL;

/// Where the clip levels lie about, a leaf cell.
const EYE: [i32; 2] = [1000 << 10, (1 << 20) + 12_345];

/// The ground's top at leaf cell `[x, y]`, in leaf cells: hills, and a
/// tower in the far quarter of every brick of the leaves (`TOWER`), so a
/// bound short of a brick misses it.
fn height([x, y]: [i32; 2]) -> i32 {
    let hills = 40.0 + 25.0 * libm::sinf(x as f32 * 0.003) + 20.0 * libm::cosf(y as f32 * 0.004);
    let tower =
        TOWER.contains(&(x & (BRICK as i32 - 1))) && TOWER.contains(&(y & (BRICK as i32 - 1)));
    hills as i32 + if tower { 300 } else { 0 }
}

/// Where a tower stands in a brick of the leaves, on each axis.
const TOWER: core::ops::Range<i32> = 40..48;

/// The clip levels as one side holds them, laid out as a GPU's are (pxl-shader
/// world.rs `World`): every clip level's entries then their first bricks
/// (`ORIGIN_AT`), the eye's clip levels' entries, and each slot's brick.
#[derive(Clone, Copy)]
struct Clipmap<'a> {
    rows: &'a [Entry],
    joined: &'a [[u32; JOINED_WORDS as usize]],
    ground: &'a [u32],
}

impl Presence for Clipmap<'_> {
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        let row = self.rows[((cell - FIRST_CELL) * ENTRIES + entry(q)) as usize];
        (row.slot(), row.holds(q))
    }
}

impl Bricks for Clipmap<'_> {
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.rows[((cell - FIRST_CELL) * ENTRIES + entry) as usize].brick()
    }
    fn palette_word(&self, _: Zoom, _: u32, _: u32) -> u32 {
        ground::word([Sample::GROUND; 4])
    }
    fn column(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        column_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn level(&self, _: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        level_of(self.ground, slot * BRICK_WORDS, j, at)
    }
    fn laid(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        laid_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn joined(&self, cell: Zoom, e: u32) -> [u32; JOINED_WORDS as usize] {
        self.joined[((cell - FIRST_CELL) * JOINED_ENTRIES + e) as usize]
    }
    fn origin(&self, cell: Zoom) -> ([i32; 2], bool) {
        self.rows[(ORIGIN_AT + cell - FIRST_CELL) as usize].brick()
    }
    fn root(&self, _: Zoom, slot: u32) -> u32 {
        root_of(self.ground, slot * BRICK_WORDS)
    }
}

/// No service area: nothing is past it.
impl Area for Clipmap<'_> {
    fn ends(&self) -> bool {
        false
    }
    fn nine(&self, _: u32) -> u32 {
        0
    }
}

impl ground::Resident for Clipmap<'_> {
    type Content = Sample;
    fn relief(
        &mut self,
        place: &mut Lookup,
        cells: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (Bounds, bool) {
        let (r, _, here) = bound_over(self, place, cells, (zoom, [c[0], c[1]]));
        (r, here)
    }
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> ground::Cell {
        clipmap::ground(self, shard, place, zoom, [c[0], c[1]]).0
    }
}

/// Nothing placed and no ways.
struct Nothing;

impl Columns for Nothing {
    fn first_child(&self, _: u32) -> u32 {
        0
    }
    fn data(&self, _: u32) -> u32 {
        VOID
    }
}

/// The world's one query over the clip levels, counting the nodes asked.
struct Counter<'a> {
    w: Clipmap<'a>,
    nodes: u32,
}

impl Query for Counter<'_> {
    type At = Inheritance;
    fn root(&mut self, node: [i32; 3]) -> Inheritance {
        Inheritance::of(&Nothing, (0, node))
    }
    fn child(&mut self, at: &Inheritance, zoom: Zoom, node: [i32; 3]) -> Inheritance {
        at.child(&Nothing, &Nothing, (zoom, node))
    }
    fn node(
        &mut self,
        at: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        zooms: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        self.nodes += 1;
        let w = self.w;
        node(
            &mut self.w,
            &w,
            &Nothing,
            &Nothing,
            at,
            (zoom, c),
            zooms,
            [Cut::NONE; 2],
        )
    }
}

/// The clip levels from the first to `CELL`, each about `EYE`, every brick
/// held but `CELL`'s east half; each held brick's columns, cells as laid
/// and levels, a slot each. Its entries are not folded.
fn clipmap() -> (Vec<Entry>, Vec<u32>) {
    let mut rows = vec![Entry::absent([0, 0]); (ORIGIN_AT + CLIP_LEVELS) as usize];
    let mut ground = Vec::new();
    for cell in FIRST_CELL..=CELL {
        let s = LEAF_CELL - cell;
        let first = [
            (EYE[0] >> s >> BRICK_SHIFT) - ACROSS as i32 / 2,
            (EYE[1] >> s >> BRICK_SHIFT) - ACROSS as i32 / 2,
        ];
        rows[(ORIGIN_AT + cell - FIRST_CELL) as usize] = Entry {
            brick: first,
            present: 1,
            ticket: 0,
        };
        for k in 0..ENTRIES {
            let q = [
                first[0] + (k % ACROSS) as i32,
                first[1] + (k / ACROSS) as i32,
            ];
            let row = &mut rows[((cell - FIRST_CELL) * ENTRIES + entry(q)) as usize];
            *row = Entry::absent(q);
            if cell == CELL && k % ACROSS >= ACROSS / 2 {
                continue;
            }
            let slot = (ground.len() as u32) / BRICK_WORDS;
            row.present = slot as i32 + 1;
            let base = slot * BRICK_WORDS;
            ground.resize(ground.len() + BRICK_WORDS as usize, 0);
            for i in 0..BRICK_CELLS {
                let [x, y] = of_idx(i);
                let mid = |v: i32| (v << s) + ((1 << s) >> 1);
                let leaf = [
                    mid((q[0] << BRICK_SHIFT) + x as i32),
                    mid((q[1] << BRICK_SHIFT) + y as i32),
                ];
                let top = height(leaf);
                put(&mut ground, base + COLUMNS_AT + i, top as u32 & 0xffff);
                let w = &mut ground[(base + LAID_AT + i / LAID_CELLS) as usize];
                *w |= (Sample::GROUND) << (8 * (i % LAID_CELLS));
                let e = base + LEVELS_AT + LEVEL_ENTRY_WORDS * level_idx(0, [x, y]);
                let b = ground::bounds_word([[top, top], ground::EMPTY]);
                put(&mut ground, e, b[0] as u32);
                put(&mut ground, e + 1, b[1] as u32);
            }
            for j in 1..=LEVELS {
                let side = BRICK >> j;
                for k in 0..side * side {
                    let at = [k % side, k / side];
                    let b = join(
                        &LoneBrick {
                            ground: &ground,
                            base,
                        },
                        0,
                        0,
                        j,
                        at,
                    );
                    let e = base + LEVELS_AT + LEVEL_ENTRY_WORDS * level_idx(j, at);
                    put(&mut ground, e, b[0] as u32);
                    put(&mut ground, e + 1, b[1] as u32);
                }
            }
        }
    }
    (rows, ground)
}

/// Each clip level's entries folded a level at a time, as `fold_cs` folds
/// them; `zero`, the zero entry, none, over a block a brick of which the
/// clip level does not hold present: no bound, which only splits more.
fn folded(rows: &[Entry], ground: &[u32], zero: bool) -> Vec<[u32; JOINED_WORDS as usize]> {
    let mut joined = vec![[0; JOINED_WORDS as usize]; (EYE_CLIP_LEVELS * JOINED_ENTRIES) as usize];
    for l in 1..=JOINED_LEVELS {
        let blocks = (ACROSS >> l) * (ACROSS >> l);
        for cell in FIRST_CELL..=CELL {
            let w = Clipmap {
                rows,
                joined: &joined,
                ground,
            };
            let entries: Vec<_> = (0..blocks)
                .map(|b| {
                    let [x, y] = of_idx(b);
                    let unheld = (0..4).any(|k| {
                        let s = entry([(2 * x + k % 2) as i32, (2 * y + k / 2) as i32]);
                        !w.brick(cell, s).1
                    });
                    if zero && l == 1 && unheld {
                        [0; JOINED_WORDS as usize]
                    } else {
                        fold(&w, cell, l, b)
                    }
                })
                .collect();
            for (b, e) in entries.into_iter().enumerate() {
                joined
                    [((cell - FIRST_CELL) * JOINED_ENTRIES + joined_idx(l, [0, 0])) as usize + b] =
                    e;
            }
        }
    }
    joined
}

/// A block of a clip level a brick of which it does not hold yet is bounded
/// by the cells a ray reads in its place, a coarser clip level's: every one of
/// them lies in the bound a node over the block reads (`clipmap::fold`).
#[test]
fn partial_block_bounded() {
    let (rows, ground) = clipmap();
    let joined = folded(&rows, &ground, false);
    let w = Clipmap {
        rows: &rows,
        joined: &joined,
        ground: &ground,
    };
    let (first, _) = w.origin(CELL);
    let (zoom, side) = (CELL - LEVELS - 1, 2 * BRICK as i32);
    let mut unheld = 0;
    for k in 0..ENTRIES / 4 {
        let c = [
            (first[0] >> 1) + (k % (ACROSS / 2)) as i32,
            (first[1] >> 1) + (k / (ACROSS / 2)) as i32,
        ];
        let q = |d: i32| [2 * c[0] + d % 2, 2 * c[1] + d / 2];
        if (0..4).all(|d| w.present(CELL, q(d)).1) {
            continue;
        }
        unheld += 1;
        let mut place = Lookup::NONE;
        let (b, _, here) = bound_over(&w, &mut place, [CELL, CELL], (zoom, c));
        assert!(
            here && b[0][0] > ground::UNBOUNDED[0],
            "{c:?} bounded: {b:?}"
        );
        for i in 0..side * side {
            let g = [c[0] * side + i % side, c[1] * side + i / side];
            let (_, column, _, f) = clipmap::cell(&w, Lookup::NONE, CELL, g);
            assert!(f.here(), "{g:?} held");
            let top = column_top(column);
            assert!(
                b[0][0] <= top && top <= b[0][1],
                "{g:?}: {top} in {:?}",
                b[0]
            );
        }
    }
    assert_eq!(unheld, ENTRIES / 8);
}

/// A column surface down any column of the half held clip level stands where it
/// stands with no bound over a block a brick of which the clip level does not
/// hold, with the same assets about it off its line, and asks fewer nodes.
#[test]
fn footing_without_bound() {
    let (rows, ground) = clipmap();
    let (none, folded) = (folded(&rows, &ground, true), folded(&rows, &ground, false));
    let world = |joined| Counter {
        w: Clipmap {
            rows: &rows,
            joined,
            ground: &ground,
        },
        nodes: 0,
    };
    let (mut was, mut is) = (world(&none), world(&folded));
    let (first, _) = was.w.origin(CELL);
    let mut hits = 0;
    // Spread over the clip level, and on every brick's tower.
    let (side, brick) = (ACROSS as i32, BRICK as i32);
    let spread = (0..CLIP_SIZE as i32)
        .step_by(29)
        .flat_map(|x| (0..CLIP_SIZE as i32).step_by(37).map(move |y| [x, y]));
    let towers = (0..side * side).map(|k| [k % side * brick + 44, k / side * brick + 44]);
    for [gx, gy] in spread.chain(towers) {
        let g = [
            (first[0] << BRICK_SHIFT) + gx,
            (first[1] << BRICK_SHIFT) + gy,
        ];
        let (a, b) = (
            column_surface(&mut was, CELL, g),
            column_surface(&mut is, CELL, g),
        );
        assert_eq!((a.top, a.ends), (b.top, b.ends), "{g:?}");
        hits += (a.ends == End::Hit) as u32;
        for d in 0..27 {
            let c = [g[0] + d % 3 - 1, g[1] + d / 3 % 3 - 1, a.top.0 + d / 9 - 1];
            assert_eq!(
                asset(&mut was, &a.line, CELL, c),
                asset(&mut is, &b.line, CELL, c),
                "{g:?} {c:?}"
            );
        }
    }
    assert!(hits > 0, "some column_surface stands on the ground");
    assert!(
        is.nodes < was.nodes,
        "{} nodes, {} with no bound",
        is.nodes,
        was.nodes
    );
}

/// The clip levels `0` with the service area the shards at `SHARD_ZOOM`
/// west of the `1`th, every row: their nines as a declaration lays them
/// (`area::nine`).
struct WestOf<'a>(Clipmap<'a>, i32);

impl Presence for WestOf<'_> {
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        self.0.present(cell, q)
    }
}

impl Bricks for WestOf<'_> {
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.0.brick(cell, entry)
    }
    fn palette_word(&self, cell: Zoom, slot: u32, i: u32) -> u32 {
        self.0.palette_word(cell, slot, i)
    }
    fn column(&self, cell: Zoom, slot: u32, i: u32) -> u32 {
        self.0.column(cell, slot, i)
    }
    fn level(&self, cell: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        self.0.level(cell, slot, j, at)
    }
    fn laid(&self, cell: Zoom, slot: u32, i: u32) -> u32 {
        self.0.laid(cell, slot, i)
    }
    fn joined(&self, cell: Zoom, e: u32) -> [u32; JOINED_WORDS as usize] {
        self.0.joined(cell, e)
    }
    fn origin(&self, cell: Zoom) -> ([i32; 2], bool) {
        self.0.origin(cell)
    }
    fn root(&self, cell: Zoom, slot: u32) -> u32 {
        self.0.root(cell, slot)
    }
}

impl Area for WestOf<'_> {
    fn ends(&self) -> bool {
        true
    }
    fn nine(&self, key: u32) -> u32 {
        // `area::shard_key` undone: X's and Y's bits interleaved from the top.
        let mut c = [0i32; 2];
        for b in 0..SHARD_ZOOM {
            c[0] |= ((key >> (2 * b) & 1) as i32) << b;
            c[1] |= ((key >> (2 * b + 1) & 1) as i32) << b;
        }
        area::nine(|n| n[0] < self.1, c)
    }
}

/// The wall is the area's alone (`area::is_wall`): a leaf column beside a
/// leaf cell past the area is the wall, bedrock, read off a stand-in brick,
/// a coarser clip level's, or off none, and at a zoom finer than the
/// leaves; one a leaf cell further in is its own ground.
#[test]
fn wall_off_any_brick() {
    let (rows, ground) = clipmap();
    let none = vec![Entry::absent([0, 0]); rows.len()];
    let shard = (EYE[0] >> (LEAF_CELL - SHARD_ZOOM)) + 1;
    // The first leaf cell past the area.
    let past = shard << (LEAF_CELL - SHARD_ZOOM);
    let read = |rows: &[Entry], zoom: Zoom, b: [i32; 2]| {
        let w = WestOf(
            Clipmap {
                rows,
                joined: &[],
                ground: &ground,
            },
            shard,
        );
        clipmap::ground(&w, Shard::UNKNOWN, Lookup::NONE, zoom, b)
    };
    let y = EYE[1];
    for (x, palette_idx) in [(past - 1, ground::UNKNOWN), (past - 2, Sample::GROUND)] {
        let (cell, f) = read(&rows, LEAF_CELL, [x, y]);
        assert!(f.here() && f.from() < LEAF_CELL, "{x} off a stand-in");
        assert_eq!(cell.palette_idx, palette_idx, "{x} off a stand-in");
        let (cell, f) = read(&none, LEAF_CELL, [x, y]);
        assert!(!f.here(), "{x} off none");
        let none_idx = if palette_idx == ground::UNKNOWN {
            ground::UNKNOWN
        } else {
            VOID
        };
        assert_eq!(cell.palette_idx, none_idx, "{x} off none");
        for d in 0..4 {
            let (cell, _) = read(&rows, LEAF_CELL + 2, [(x << 2) + d, (y << 2) + d]);
            assert_eq!(cell.palette_idx, palette_idx, "{x}'s quarter {d}");
        }
    }
}
