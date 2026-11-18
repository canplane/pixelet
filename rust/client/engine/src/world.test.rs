// pxl-engine/world.test.rs

use super::*;
use crate::cast::{End, Query};
use crate::clipmap;
use crate::clipmap::{
    ACROSS, BOUND_ENTRY_WORDS, BOUNDS_AT, BRICK_MIP_LAST, BRICK_WORDS, CLIP_LEVEL_CNT, CLIP_SIZE,
    COLUMNS_AT, ENTRY_CNT, EYE_CLIP_LEVEL_CNT, Entry, FIRST_CELL_ZOOM, JOINED_ENTRY_CNT,
    JOINED_MIP_CNT, JOINED_WORDS, ORIGIN_AT, OWN_AT, OWN_CELL_CNT, Pool, bound_over, entry, fold,
    join, joined_idx, mip_idx,
};
use crate::content::Sample;
use crate::fill::{LoneBrick, column_of, mip_of, own_of, put, root_of};
use crate::ground::Extent;
use crate::support::column_surface;
use pxl_spec::brick::{BRICK, BRICK_CELL_CNT, BRICK_SHIFT, of_idx};
use pxl_spec::ground::{GroundId, word};
use pxl_spec::space::coord::zoom::SHARD_ZOOM;
use pxl_spec::space::hierarchy::tree::ancestor_at;
use pxl_spec::space::layer::area;
use pxl_spec::space::layer::area::{Area, Shard};
use xpute_kit::math::rng::mix64;

/// The clip level whose bricks are only half held, its east half not yet, with the clip
/// levels over it held whole. It is the leaves' level, which decor's column surfaces ask, and
/// where a node wider than a brick may stand clear over the ground.
const CELL_ZOOM: Zoom = LEAF_CELL_ZOOM;

/// The leaf cell the clip levels lie around.
const EYE: [i32; 2] = [1000 << 10, (1 << 20) + 12_345];

/// The ground's top at leaf cell `[x, y]`, in leaf cells: hills, and a tower in the far
/// quarter of every brick of the leaves (`TOWER`).
fn ele_at([x, y]: [i32; 2]) -> i32 {
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

impl BrickSource for Clipmap<'_> {
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool) {
        let row = self.rows[((cell_zoom - FIRST_CELL_ZOOM) * ENTRY_CNT + entry(q)) as usize];
        (row.slot(), row.holds(q))
    }
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.rows[((cell_zoom - FIRST_CELL_ZOOM) * ENTRY_CNT + entry) as usize].brick()
    }
    fn fold_at(&self, _: Zoom, _: u32, _: u32) -> u32 {
        word([Sample::GROUND; 4])
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
        self.joined[((cell_zoom - FIRST_CELL_ZOOM) * JOINED_ENTRY_CNT + e) as usize]
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
    type Place = Lookup;
    const NOWHERE: Lookup = Lookup::NONE;
    fn relief(
        &mut self,
        place: &mut Lookup,
        cell_range: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (Extent, bool) {
        let (r, _, here) = bound_over(self, place, cell_range, (zoom, [c[0], c[1]]));
        (r, here)
    }
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> ground::Cell {
        clipmap::ground(self, shard, place, zoom, [c[0], c[1]]).0
    }
}

/// Nothing placed and no ways.
struct Nothing;

impl Pool for Nothing {
    fn word(&self, _: u32) -> u32 {
        0
    }
}

impl Table for Nothing {
    fn first_child(&self, _: u32) -> u32 {
        0
    }
    fn data(&self, _: u32) -> u32 {
        Sentinel::Void as u32
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
        at.child(&Nothing, (zoom, node))
    }
    fn node(
        &mut self,
        at: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        range: [Zoom; 2],
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
            range,
            [Cut::NONE; 2],
        )
    }
}

/// The clip levels from the first to `CELL_ZOOM`, each around `EYE`, with every brick held
/// except the east half of `CELL_ZOOM`'s, and each held brick's columns, cells as laid and
/// levels, one slot each. The entries are not folded.
fn clipmap() -> (Vec<Entry>, Vec<u32>) {
    let mut rows = vec![Entry::absent([0, 0]); (ORIGIN_AT + CLIP_LEVEL_CNT) as usize];
    let mut ground = Vec::new();
    for cell_zoom in FIRST_CELL_ZOOM..=CELL_ZOOM {
        let s = LEAF_CELL_ZOOM - cell_zoom;
        let first = [
            (EYE[0] >> s >> BRICK_SHIFT) - ACROSS as i32 / 2,
            (EYE[1] >> s >> BRICK_SHIFT) - ACROSS as i32 / 2,
        ];
        rows[(ORIGIN_AT + cell_zoom - FIRST_CELL_ZOOM) as usize] = Entry::at(first, 1);
        for k in 0..ENTRY_CNT {
            let q = [
                first[0] + (k % ACROSS) as i32,
                first[1] + (k / ACROSS) as i32,
            ];
            let row = &mut rows[((cell_zoom - FIRST_CELL_ZOOM) * ENTRY_CNT + entry(q)) as usize];
            *row = Entry::absent(q);
            if cell_zoom == CELL_ZOOM && k % ACROSS >= ACROSS / 2 {
                continue;
            }
            let slot = (ground.len() as u32) / BRICK_WORDS;
            row.present = slot as i32 + 1;
            let base = slot * BRICK_WORDS;
            ground.resize(ground.len() + BRICK_WORDS as usize, 0);
            for i in 0..BRICK_CELL_CNT {
                let [x, y] = of_idx(i);
                let mid = |v: i32| (v << s) + ((1 << s) >> 1);
                let leaf = [
                    mid((q[0] << BRICK_SHIFT) + x as i32),
                    mid((q[1] << BRICK_SHIFT) + y as i32),
                ];
                let top = ele_at(leaf);
                put(&mut ground, base + COLUMNS_AT + i, top as u32 & 0xffff);
                let w = &mut ground[(base + OWN_AT + i / OWN_CELL_CNT) as usize];
                *w |= (Sample::GROUND) << (GroundId::BITS * (i % OWN_CELL_CNT));
                let e = base + BOUNDS_AT + BOUND_ENTRY_WORDS * mip_idx(0, [x, y]);
                let b = ground::extent_word([[top, top], ground::EMPTY]);
                put(&mut ground, e, b[0] as u32);
                put(&mut ground, e + 1, b[1] as u32);
            }
            for j in 1..=BRICK_MIP_LAST {
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
/// (`clipmap::fold`).
fn folded(rows: &[Entry], ground: &[u32]) -> Vec<[u32; JOINED_WORDS as usize]> {
    let mut joined =
        vec![[0; JOINED_WORDS as usize]; (EYE_CLIP_LEVEL_CNT * JOINED_ENTRY_CNT) as usize];
    for l in 1..=JOINED_MIP_CNT {
        let blocks = (ACROSS >> l) * (ACROSS >> l);
        for cell_zoom in FIRST_CELL_ZOOM..=CELL_ZOOM {
            let w = Clipmap {
                rows,
                joined: &joined,
                ground,
            };
            let entries: Vec<_> = (0..blocks).map(|b| fold(&w, cell_zoom, l, b)).collect();
            for (b, e) in entries.into_iter().enumerate() {
                joined[((cell_zoom - FIRST_CELL_ZOOM) * JOINED_ENTRY_CNT + joined_idx(l, [0, 0]))
                    as usize
                    + b] = e;
            }
        }
    }
    joined
}

/// The clip levels `0` with a service area of the shards at `SHARD_ZOOM` west of the `1`th
/// in every row, each cell's nine bits as a declaration lays them (`area::nine`).
struct WestOf<'a>(Clipmap<'a>, i32);

impl BrickSource for WestOf<'_> {
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool) {
        self.0.present(cell_zoom, q)
    }
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.0.brick(cell_zoom, entry)
    }
    fn fold_at(&self, cell_zoom: Zoom, slot: u32, i: u32) -> u32 {
        self.0.fold_at(cell_zoom, slot, i)
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
    let unknown = Sentinel::Unknown as u32;
    for (x, asset_id) in [(past - 1, unknown), (past - 2, Sample::GROUND)] {
        let (cell, f) = read(&rows, LEAF_CELL_ZOOM, [x, y]);
        assert!(f.here() && f.from() < LEAF_CELL_ZOOM, "{x} off a stand-in");
        assert_eq!(cell.asset_id, asset_id, "{x} off a stand-in");
        let (cell, f) = read(&none, LEAF_CELL_ZOOM, [x, y]);
        assert!(!f.here(), "{x} off none");
        let none_idx = if asset_id == unknown {
            unknown
        } else {
            Sentinel::Void as u32
        };
        assert_eq!(cell.asset_id, none_idx, "{x} off none");
        for d in 0..4 {
            let (cell, _) = read(&rows, LEAF_CELL_ZOOM + 2, [(x << 2) + d, (y << 2) + d]);
            assert_eq!(cell.asset_id, asset_id, "{x}'s quarter {d}");
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
    let joined = folded(&rows, &ground);
    let mut unheld = rows.clone();
    for e in unheld
        .iter_mut()
        .take(((CELL_ZOOM - FIRST_CELL_ZOOM + 1) * ENTRY_CNT) as usize)
    {
        *e = Entry::absent(e.brick().0);
    }
    let unheld_joined = folded(&unheld, &ground);
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
        kinds[Kind::Cell as usize] > 0 && kinds[Kind::Lack as usize] > 0,
        "{kinds:?}"
    );
}

/// `cell_answer_takes_no_zooms` over the clip levels `w`, returning how many answers of each
/// kind it asked.
fn answers_take_no_zooms(w: Clipmap<'_>) -> [u32; 3] {
    let mut resident = w;
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
        // Around the ground's top where the column meets it, or else around its elevation.
        let surface = column_surface(&mut asking, CELL_ZOOM, g);
        let top = if surface.end == End::Hit {
            surface.top.0
        } else {
            ele_at(g)
        };
        for dz in -2..=1 {
            let leaf = [g[0], g[1], top + dz];
            for zoom in FIRST_CELL_ZOOM..=CELL_ZOOM {
                let c = ancestor_at(CELL_ZOOM, leaf, CELL_ZOOM - zoom);
                let mut alone = Inheritance::of(&Nothing, (zoom, c));
                let base = node(
                    &mut resident,
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
                    let mut at = Inheritance::of(&Nothing, (0, ancestor_at(zoom, c, zoom)));
                    let mut reached = true;
                    for z in 0..zoom {
                        let a = ancestor_at(zoom, c, zoom - z);
                        let near = z + 1 + take(CELL_ZOOM + 2 - z);
                        let far = take(near + 1);
                        let got = node(
                            &mut resident,
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
                        let child = ancestor_at(zoom, c, zoom - z - 1);
                        at = at.child(&Nothing, (z + 1, child));
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
                        &mut resident,
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
