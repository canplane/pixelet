// pxl-spec-core/world.test.rs

extern crate std;

use super::*;
use crate::area::Area;
use crate::cast::{End, Query};
use crate::cube::ground::stone;
use crate::fill::{column_of, laid_of, level_of, put, root_of, Fresh};
use crate::ground::Bounds;
use crate::support::{asset, footing};
use crate::window::{
    self, bound_over, column_top, entry, fold, join, joined_idx, laid_half, level_idx, of_idx,
    Entry, ACROSS, BRICK, BRICK_CELLS, BRICK_SHIFT, BRICK_WORDS, COLUMNS_AT, ENTRIES, EYE_WINDOWS,
    FIRST_CELL, JOINED_ENTRIES, JOINED_LEVELS, JOINED_WORDS, LAID_AT, LEVELS, LEVELS_AT,
    LEVEL_ENTRY_WORDS, ORIGIN_AT, WINDOW, WINDOWS,
};
use std::{vec, vec::Vec};

/// The window whose bricks are half held, its east half not yet, the
/// windows over it held whole: the leaves', as decor's footings ask, where
/// a node wider than a brick may stand clear over the ground.
const CELL: Zoom = LEAF_CELL;

/// Where the windows lie about, a leaf cell.
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

/// The windows as one side holds them, laid out as a GPU's are (pxl-shader
/// world.rs `World`): every window's entries then their first bricks
/// (`ORIGIN_AT`), the eye's windows' entries, and each slot's brick.
#[derive(Clone, Copy)]
struct Windows<'a> {
    rows: &'a [Entry],
    joined: &'a [[u32; JOINED_WORDS as usize]],
    ground: &'a [u32],
}

impl Bricks for Windows<'_> {
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        let row = self.rows[((cell - FIRST_CELL) * ENTRIES + entry(q)) as usize];
        (row.slot(), row.holds(q))
    }
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.rows[((cell - FIRST_CELL) * ENTRIES + entry) as usize].held()
    }
    fn look(&self, _: Zoom, _: u32, _: u32) -> u32 {
        ground::word([stone as u32; 4])
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
        self.rows[(ORIGIN_AT + cell - FIRST_CELL) as usize].held()
    }
    fn root(&self, _: Zoom, slot: u32) -> u32 {
        root_of(self.ground, slot * BRICK_WORDS)
    }
}

/// No service area: nothing is past it.
impl Area for Windows<'_> {
    fn ends(&self) -> bool {
        false
    }
    fn nine(&self, _: u32) -> u32 {
        0
    }
}

impl Held for Windows<'_> {
    fn relief(
        &mut self,
        place: &mut Found,
        cells: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (Bounds, bool) {
        let (r, _, here) = bound_over(self, place, cells, (zoom, [c[0], c[1]]));
        (r, here)
    }
    fn cell(&mut self, place: Found, zoom: Zoom, c: [i32; 3]) -> ground::Cell {
        window::ground(self, place, zoom, [c[0], c[1]]).0
    }
}

/// Nothing placed and no ways.
struct Nothing;

impl Lanes for Nothing {
    fn heap(&self, _: u32) -> u32 {
        0
    }
    fn data(&self, _: u32) -> u32 {
        VOID
    }
}

/// The world's one query over the windows, counting the nodes asked.
struct Asked<'a> {
    w: Windows<'a>,
    nodes: u32,
}

impl Query for Asked<'_> {
    type At = Handed;
    fn root(&mut self, node: [i32; 3]) -> Handed {
        Handed::of(&Nothing, (0, node))
    }
    fn child(&mut self, at: &Handed, zoom: Zoom, node: [i32; 3]) -> Handed {
        at.child(&Nothing, &Nothing, (zoom, node))
    }
    fn node(
        &mut self,
        at: &mut Handed,
        zoom: Zoom,
        c: [i32; 3],
        asked: [Zoom; 2],
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
            asked,
            [Cut::NONE; 2],
        )
    }
}

/// The windows from the first to `CELL`, each about `EYE`, every brick
/// held but `CELL`'s east half; each held brick's columns, cells as laid
/// and levels, a slot each. Its entries are not folded.
fn windows() -> (Vec<Entry>, Vec<u32>) {
    let mut rows = vec![Entry::absent([0, 0]); (ORIGIN_AT + WINDOWS) as usize];
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
                let w = &mut ground[(base + LAID_AT + i / 2) as usize];
                *w |= laid_half(stone as u32, 0) << (16 * (i % 2));
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
                        &Fresh {
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

/// Each window's entries folded a level at a time, as `joined_cs` folds
/// them; `zero`, the zero entry, none, over a block a brick of which the
/// window does not hold present: no bound, which only splits more.
fn folded(rows: &[Entry], ground: &[u32], zero: bool) -> Vec<[u32; JOINED_WORDS as usize]> {
    let mut joined = vec![[0; JOINED_WORDS as usize]; (EYE_WINDOWS * JOINED_ENTRIES) as usize];
    for l in 1..=JOINED_LEVELS {
        let blocks = (ACROSS >> l) * (ACROSS >> l);
        for cell in FIRST_CELL..=CELL {
            let w = Windows {
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

/// A block of a window a brick of which it does not hold yet is bounded
/// by the cells a ray reads in its place, a coarser window's: every one of
/// them lies in the bound a node over the block reads (`window::fold`).
#[test]
fn partial_block_bounded() {
    let (rows, ground) = windows();
    let joined = folded(&rows, &ground, false);
    let w = Windows {
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
        let mut place = Found::NONE;
        let (b, _, here) = bound_over(&w, &mut place, [CELL, CELL], (zoom, c));
        assert!(
            here && b[0][0] > ground::UNBOUNDED[0],
            "{c:?} bounded: {b:?}"
        );
        for i in 0..side * side {
            let g = [c[0] * side + i % side, c[1] * side + i / side];
            let (_, column, _, f) = window::cell(&w, Found::NONE, CELL, g);
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

/// A footing down any column of the half held window stands where it
/// stands with no bound over a block a brick of which the window does not
/// hold, with the same assets about it off its line, and asks fewer nodes.
#[test]
fn footing_without_bound() {
    let (rows, ground) = windows();
    let (none, folded) = (folded(&rows, &ground, true), folded(&rows, &ground, false));
    let world = |joined| Asked {
        w: Windows {
            rows: &rows,
            joined,
            ground: &ground,
        },
        nodes: 0,
    };
    let (mut was, mut is) = (world(&none), world(&folded));
    let (first, _) = was.w.origin(CELL);
    let mut stops = 0;
    // Spread over the window, and on every brick's tower.
    let (side, brick) = (ACROSS as i32, BRICK as i32);
    let spread = (0..WINDOW as i32)
        .step_by(29)
        .flat_map(|x| (0..WINDOW as i32).step_by(37).map(move |y| [x, y]));
    let towers = (0..side * side).map(|k| [k % side * brick + 44, k / side * brick + 44]);
    for [gx, gy] in spread.chain(towers) {
        let g = [
            (first[0] << BRICK_SHIFT) + gx,
            (first[1] << BRICK_SHIFT) + gy,
        ];
        let (a, b) = (footing(&mut was, CELL, g), footing(&mut is, CELL, g));
        assert_eq!((a.top, a.ends), (b.top, b.ends), "{g:?}");
        stops += (a.ends == End::Stop) as u32;
        for d in 0..27 {
            let c = [g[0] + d % 3 - 1, g[1] + d / 3 % 3 - 1, a.top.0 + d / 9 - 1];
            assert_eq!(
                asset(&mut was, &a.line, CELL, c),
                asset(&mut is, &b.line, CELL, c),
                "{g:?} {c:?}"
            );
        }
    }
    assert!(stops > 0, "some footing stands on the ground");
    assert!(
        is.nodes < was.nodes,
        "{} nodes, {} with no bound",
        is.nodes,
        was.nodes
    );
}
