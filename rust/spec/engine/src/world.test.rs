// pxl-engine/world.test.rs

extern crate std;

use super::*;
use crate::area::{self, Area, Shard};
use crate::cast::{End, Query};
use crate::clipmap::{
    self, bound_over, column_top, entry, fold, join, joined_idx, mip_idx, of_idx, Entry, Presence,
    ACROSS, BOUNDS_AT, BOUND_ENTRY_WORDS, BRICK, BRICK_CELLS, BRICK_MIPS, BRICK_SHIFT, BRICK_WORDS,
    CLIP_LEVELS, CLIP_SIZE, COLUMNS_AT, ENTRIES, EYE_CLIP_LEVELS, FIRST_CELL_ZOOM, JOINED_ENTRIES,
    JOINED_MIPS, JOINED_WORDS, ORIGIN_AT, OWN_AT, OWN_CELLS,
};
use crate::content::Sample;
use crate::fill::{column_of, mip_of, own_of, put, root_of, LoneBrick};
use crate::ground::Bounds;
use crate::support::{asset, column_surface};
use crate::SHARD_ZOOM;
use std::{vec, vec::Vec};
use xpute_kit::math::rng::mix64;

/// The clip level whose bricks are only half held, its east half not yet, with the clip
/// levels over it held whole. It is the leaves' level, which decor's column surfaces ask, and
/// where a node wider than a brick may stand clear over the ground.
const CELL_ZOOM: Zoom = LEAF_CELL_ZOOM;

/// The leaf cell the clip levels lie around.
const EYE: [i32; 2] = [1000 << 10, (1 << 20) + 12_345];

/// The ground's top at leaf cell `[x, y]`, in leaf cells: hills, and a tower in the far
/// quarter of every brick of the leaves (`TOWER`), so that a bound that falls short of a
/// whole brick misses the tower.
fn height([x, y]: [i32; 2]) -> i32 {
    let hills = 40.0 + 25.0 * libm::sinf(x as f32 * 0.003) + 20.0 * libm::cosf(y as f32 * 0.004);
    let tower =
        TOWER.contains(&(x & (BRICK as i32 - 1))) && TOWER.contains(&(y & (BRICK as i32 - 1)));
    hills as i32 + if tower { 300 } else { 0 }
}

/// Where a tower stands in a brick of the leaves, on each axis.
const TOWER: core::ops::Range<i32> = 40..48;

/// The clip levels as one side holds them, laid out as a GPU's are (pxl-render clip.wgsl):
/// every clip level's entries and then their first bricks (`ORIGIN_AT`), the eye's clip
/// levels' joined entries, and each slot's brick.
#[derive(Clone, Copy)]
struct Clipmap<'a> {
    rows: &'a [Entry],
    joined: &'a [[u32; JOINED_WORDS as usize]],
    ground: &'a [u32],
}

impl Presence for Clipmap<'_> {
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool) {
        let row = self.rows[((cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + entry(q)) as usize];
        (row.slot(), row.holds(q))
    }
}

impl Bricks for Clipmap<'_> {
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.rows[((cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + entry) as usize].brick()
    }
    fn palette_word(&self, _: Zoom, _: u32, _: u32) -> u32 {
        ground::word([Sample::GROUND; 4])
    }
    fn column(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        column_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn bound_mip(&self, _: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        mip_of(self.ground, slot * BRICK_WORDS, j, at)
    }
    fn own(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        own_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn joined(&self, cell_zoom: Zoom, e: u32) -> [u32; JOINED_WORDS as usize] {
        self.joined[((cell_zoom - FIRST_CELL_ZOOM) * JOINED_ENTRIES + e) as usize]
    }
    fn origin(&self, cell_zoom: Zoom) -> ([i32; 2], bool) {
        self.rows[(ORIGIN_AT + cell_zoom - FIRST_CELL_ZOOM) as usize].brick()
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

/// The world's one query over the clip levels.
struct Asking<'a> {
    w: Clipmap<'a>,
}

impl Query for Asking<'_> {
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

/// The clip levels from the first to `CELL_ZOOM`, each around `EYE`, with every brick held
/// except the east half of `CELL_ZOOM`'s, and each held brick's columns, cells as laid and
/// levels, one slot each. The entries are not folded.
fn clipmap() -> (Vec<Entry>, Vec<u32>) {
    let mut rows = vec![Entry::absent([0, 0]); (ORIGIN_AT + CLIP_LEVELS) as usize];
    let mut ground = Vec::new();
    for cell_zoom in FIRST_CELL_ZOOM..=CELL_ZOOM {
        let s = LEAF_CELL_ZOOM - cell_zoom;
        let first = [
            (EYE[0] >> s >> BRICK_SHIFT) - ACROSS as i32 / 2,
            (EYE[1] >> s >> BRICK_SHIFT) - ACROSS as i32 / 2,
        ];
        rows[(ORIGIN_AT + cell_zoom - FIRST_CELL_ZOOM) as usize] = Entry::at(first, 1);
        for k in 0..ENTRIES {
            let q = [
                first[0] + (k % ACROSS) as i32,
                first[1] + (k / ACROSS) as i32,
            ];
            let row = &mut rows[((cell_zoom - FIRST_CELL_ZOOM) * ENTRIES + entry(q)) as usize];
            *row = Entry::absent(q);
            if cell_zoom == CELL_ZOOM && k % ACROSS >= ACROSS / 2 {
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
                let w = &mut ground[(base + OWN_AT + i / OWN_CELLS) as usize];
                *w |= (Sample::GROUND) << (8 * (i % OWN_CELLS));
                let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * mip_idx(0, [x, y]);
                let b = ground::bounds_word([[top, top], ground::EMPTY]);
                put(&mut ground, e, b[0] as u32);
                put(&mut ground, e + 1, b[1] as u32);
            }
            for j in 1..=BRICK_MIPS {
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
                    let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * mip_idx(j, at);
                    put(&mut ground, e, b[0] as u32);
                    put(&mut ground, e + 1, b[1] as u32);
                }
            }
        }
    }
    (rows, ground)
}

/// Each clip level's entries folded a level at a time, as the core folds them
/// (`clipmap::fold`). Where `zero` is set, a block with a brick the clip level does not hold
/// present gets the zero entry instead: no bound, which only makes rays split more.
fn folded(rows: &[Entry], ground: &[u32], zero: bool) -> Vec<[u32; JOINED_WORDS as usize]> {
    let mut joined = vec![[0; JOINED_WORDS as usize]; (EYE_CLIP_LEVELS * JOINED_ENTRIES) as usize];
    for l in 1..=JOINED_MIPS {
        let blocks = (ACROSS >> l) * (ACROSS >> l);
        for cell_zoom in FIRST_CELL_ZOOM..=CELL_ZOOM {
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
                        !w.brick(cell_zoom, s).1
                    });
                    if zero && l == 1 && unheld {
                        [0; JOINED_WORDS as usize]
                    } else {
                        fold(&w, cell_zoom, l, b)
                    }
                })
                .collect();
            for (b, e) in entries.into_iter().enumerate() {
                joined[((cell_zoom - FIRST_CELL_ZOOM) * JOINED_ENTRIES + joined_idx(l, [0, 0]))
                    as usize
                    + b] = e;
            }
        }
    }
    joined
}

/// A block with a brick its clip level does not hold yet is bounded by the cells a ray reads
/// in the brick's place, a coarser clip level's: every one of them lies within the bound that
/// a node over the block reads (`clipmap::fold`).
#[test]
fn partial_block_bounded() {
    let (rows, ground) = clipmap();
    let joined = folded(&rows, &ground, false);
    let w = Clipmap {
        rows: &rows,
        joined: &joined,
        ground: &ground,
    };
    let (first, _) = w.origin(CELL_ZOOM);
    let (zoom, side) = (CELL_ZOOM - BRICK_MIPS - 1, 2 * BRICK as i32);
    let mut unheld = 0;
    for k in 0..ENTRIES / 4 {
        let c = [
            (first[0] >> 1) + (k % (ACROSS / 2)) as i32,
            (first[1] >> 1) + (k / (ACROSS / 2)) as i32,
        ];
        let q = |d: i32| [2 * c[0] + d % 2, 2 * c[1] + d / 2];
        if (0..4).all(|d| w.present(CELL_ZOOM, q(d)).1) {
            continue;
        }
        unheld += 1;
        let mut place = Lookup::NONE;
        let (b, _, here) = bound_over(&w, &mut place, [CELL_ZOOM, CELL_ZOOM], (zoom, c));
        assert!(
            here && b[0][0] > ground::UNBOUNDED[0],
            "{c:?} bounded: {b:?}"
        );
        for i in 0..side * side {
            let g = [c[0] * side + i % side, c[1] * side + i / side];
            let (_, column, _, f) = clipmap::cell(&w, Lookup::NONE, CELL_ZOOM, g);
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

/// A column surface found down any column of the half-held clip level stands where it
/// stands when a block with an unheld brick has no bound, with the same assets around it off
/// its line.
#[test]
fn footing_without_bound() {
    let (rows, ground) = clipmap();
    let (none, folded) = (folded(&rows, &ground, true), folded(&rows, &ground, false));
    let world = |joined| Asking {
        w: Clipmap {
            rows: &rows,
            joined,
            ground: &ground,
        },
    };
    let (mut was, mut is) = (world(&none), world(&folded));
    let (first, _) = was.w.origin(CELL_ZOOM);
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
            column_surface(&mut was, CELL_ZOOM, g),
            column_surface(&mut is, CELL_ZOOM, g),
        );
        assert_eq!((a.top, a.ends), (b.top, b.ends), "{g:?}");
        hits += (a.ends == End::Hit) as u32;
        for d in 0..27 {
            let c = [g[0] + d % 3 - 1, g[1] + d / 3 % 3 - 1, a.top.0 + d / 9 - 1];
            assert_eq!(
                asset(&mut was, &a.line, CELL_ZOOM, c),
                asset(&mut is, &b.line, CELL_ZOOM, c),
                "{g:?} {c:?}"
            );
        }
    }
    assert!(hits > 0, "some column_surface stands on the ground");
}

/// The clip levels `0` with a service area of the shards at `SHARD_ZOOM` west of the `1`th
/// in every row, each cell's nine bits as a declaration lays them (`area::nine`).
struct WestOf<'a>(Clipmap<'a>, i32);

impl Presence for WestOf<'_> {
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool) {
        self.0.present(cell_zoom, q)
    }
}

impl Bricks for WestOf<'_> {
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.0.brick(cell_zoom, entry)
    }
    fn palette_word(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32 {
        self.0.palette_word(cell_zoom, slot, i)
    }
    fn column(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32 {
        self.0.column(cell_zoom, slot, i)
    }
    fn bound_mip(&self, cell_zoom: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        self.0.bound_mip(cell_zoom, slot, j, at)
    }
    fn own(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32 {
        self.0.own(cell_zoom, slot, i)
    }
    fn joined(&self, cell_zoom: Zoom, e: u32) -> [u32; JOINED_WORDS as usize] {
        self.0.joined(cell_zoom, e)
    }
    fn origin(&self, cell_zoom: Zoom) -> ([i32; 2], bool) {
        self.0.origin(cell_zoom)
    }
    fn root(&self, cell_zoom: Zoom, slot: u32) -> u32 {
        self.0.root(cell_zoom, slot)
    }
}

impl Area for WestOf<'_> {
    fn ends(&self) -> bool {
        true
    }
    fn nine(&self, key: u32) -> u32 {
        // `area::shard_key` undone: the bits of X and Y interleaved from the top.
        let mut c = [0i32; 2];
        for b in 0..SHARD_ZOOM {
            c[0] |= ((key >> (2 * b) & 1) as i32) << b;
            c[1] |= ((key >> (2 * b + 1) & 1) as i32) << b;
        }
        area::nine(|n| n[0] < self.1, c)
    }
}

/// The wall is the area's alone (`area::is_wall`). A leaf column beside a leaf cell past the
/// area is the wall, bedrock, whether it is read from a stand-in brick of a coarser clip
/// level or from none, and at a zoom finer than the leaves too. A column one leaf cell
/// further in is its own ground.
#[test]
fn wall_off_any_brick() {
    let (rows, ground) = clipmap();
    let none = vec![Entry::absent([0, 0]); rows.len()];
    let shard = (EYE[0] >> (LEAF_CELL_ZOOM - SHARD_ZOOM)) + 1;
    // The first leaf cell past the area.
    let past = shard << (LEAF_CELL_ZOOM - SHARD_ZOOM);
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
        let (cell, f) = read(&rows, LEAF_CELL_ZOOM, [x, y]);
        assert!(f.here() && f.from() < LEAF_CELL_ZOOM, "{x} off a stand-in");
        assert_eq!(cell.palette_idx, palette_idx, "{x} off a stand-in");
        let (cell, f) = read(&none, LEAF_CELL_ZOOM, [x, y]);
        assert!(!f.here(), "{x} off none");
        let none_idx = if palette_idx == ground::UNKNOWN {
            ground::UNKNOWN
        } else {
            VOID
        };
        assert_eq!(cell.palette_idx, none_idx, "{x} off none");
        for d in 0..4 {
            let (cell, _) = read(&rows, LEAF_CELL_ZOOM + 2, [(x << 2) + d, (y << 2) + d]);
            assert_eq!(cell.palette_idx, palette_idx, "{x}'s quarter {d}");
        }
    }
}

/// A cell's answer takes no zooms asked. Take a node the query answers as a cell or as the
/// lack, at or past the finest zoom asked (`near`), or whole past the ground's cells. It has
/// one answer whatever zooms it and every node over it were asked at, whatever its ancestors
/// handed it (among that, the ground's cells' place, `Lookup`), and the same answer with
/// nothing handed down; so does what it hands on. So a stopped cell's answer belongs to its
/// node alone (pxl-engine `record`, `node_cache`). The test runs over the clip levels held and
/// over clip levels holding no brick, where every cell lacks.
#[test]
fn cell_answer_takes_no_zooms() {
    let (rows, ground) = clipmap();
    let joined = folded(&rows, &ground, false);
    let mut unheld = rows.clone();
    for e in unheld
        .iter_mut()
        .take(((CELL_ZOOM - FIRST_CELL_ZOOM + 1) * ENTRIES) as usize)
    {
        *e = Entry::absent(e.brick().0);
    }
    let unheld_joined = folded(&unheld, &ground, true);
    let mut kinds = [0u32; 3];
    for (rows, joined) in [(&rows, &joined), (&unheld, &unheld_joined)] {
        let w = Clipmap {
            rows,
            joined,
            ground: &ground,
        };
        let got = answers_take_no_zooms(w);
        assert!(got.iter().sum::<u32>() > 1000, "{got:?} cells reached");
        for (k, n) in got.iter().enumerate() {
            kinds[k] += n;
        }
    }
    assert!(
        kinds[Kind::Cell as usize] > 0 && kinds[Kind::Lacks as usize] > 0,
        "{kinds:?}"
    );
}

/// `cell_answer_takes_no_zooms` over the clip levels `w`, returning how many answers of each
/// kind it asked.
fn answers_take_no_zooms(w: Clipmap<'_>) -> [u32; 3] {
    let mut held = w;
    let mut asking = Asking { w };
    let (first, _) = w.origin(CELL_ZOOM);
    let mut kinds = [0u32; 3];
    let spread = (0..CLIP_SIZE as i32)
        .step_by(53)
        .flat_map(|x| (0..CLIP_SIZE as i32).step_by(61).map(move |y| [x, y]));
    for (k, [gx, gy]) in spread.enumerate() {
        let g = [
            (first[0] << BRICK_SHIFT) + gx,
            (first[1] << BRICK_SHIFT) + gy,
        ];
        // Around the ground's top where the column meets it, or else around its height.
        let surface = column_surface(&mut asking, CELL_ZOOM, g);
        let top = if surface.ends == End::Hit {
            surface.top.0
        } else {
            height(g)
        };
        for dz in -2..=1 {
            let leaf = [g[0], g[1], top + dz];
            for zoom in FIRST_CELL_ZOOM..=CELL_ZOOM {
                let c = object::ancestor_at(CELL_ZOOM, leaf, CELL_ZOOM - zoom);
                let mut alone = Inheritance::of(&Nothing, (zoom, c));
                let base = node(
                    &mut held,
                    &w,
                    &Nothing,
                    &Nothing,
                    &mut alone,
                    (zoom, c),
                    [zoom, zoom],
                    [Cut::NONE; 2],
                );
                for trial in 0..12u64 {
                    let mut r = mix64(
                        (k as u64) << 20 ^ (zoom as u64) << 12 ^ ((dz + 2) as u64) << 8 ^ trial,
                    );
                    let mut take = |n: u32| {
                        let v = (r % u64::from(n)) as u32;
                        r = mix64(r);
                        v
                    };
                    // Every node over it is asked at zooms that split it, each at zooms of its own, as a
                    // walk's nodes are.
                    let mut at = Inheritance::of(&Nothing, (0, object::ancestor_at(zoom, c, zoom)));
                    let mut reached = true;
                    for z in 0..zoom {
                        let a = object::ancestor_at(zoom, c, zoom - z);
                        let near = z + 1 + take(CELL_ZOOM + 2 - z);
                        let far = take(near + 1);
                        let got = node(
                            &mut held,
                            &w,
                            &Nothing,
                            &Nothing,
                            &mut at,
                            (z, a),
                            [far, near],
                            [Cut::NONE; 2],
                        );
                        if got.kind != Kind::Split {
                            reached = false;
                            break;
                        }
                        let child = object::ancestor_at(zoom, c, zoom - z - 1);
                        at = at.child(&Nothing, &Nothing, (z + 1, child));
                    }
                    if !reached {
                        continue;
                    }
                    // Asked at zooms at which it is a cell: its own zoom or coarser, or, past the ground's
                    // cells, any zoom, where nothing placed or carved makes it whole.
                    let most = if zoom >= LEAF_CELL_ZOOM {
                        CELL_ZOOM + 3
                    } else {
                        zoom
                    };
                    let near = take(most + 1);
                    let far = take(near + 1);
                    let mut mine = at;
                    let got = node(
                        &mut held,
                        &w,
                        &Nothing,
                        &Nothing,
                        &mut mine,
                        (zoom, c),
                        [far, near],
                        [Cut::NONE; 2],
                    );
                    assert_eq!(got, base, "{c:?} at {zoom}, asked [{far}, {near}]");
                    assert_eq!(
                        mine.words(),
                        alone.words(),
                        "{c:?} at {zoom} hands on, asked [{far}, {near}]"
                    );
                    kinds[got.kind as usize] += 1;
                }
            }
        }
    }
    kinds
}
