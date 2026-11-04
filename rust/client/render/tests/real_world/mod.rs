//! A real world's buffers made on the CPU, as the fill and the store lay them, for what is
//! tested over the world's one query by hand (tests/world.rs, tests/light_miss.rs): its clip
//! levels and their bricks, the service area's index, what is placed and the ways' pool.

#![allow(dead_code, reason = "each test reads its own of it")]

use pxl_content::cube::ground as palette;
use pxl_content::Pixelet;
use pxl_engine::area::{self, Area, Shard};
use pxl_engine::clipmap::{
    self as clip, Bricks, Lookup, Presence, ACROSS, BRICK_CELLS, BRICK_SHIFT, BRICK_WORDS, ENTRIES,
    EYE_CLIP_LEVELS, FIRST_CELL, HANDED_WORDS, JOINED_AT, JOINED_ENTRIES, JOINED_LEVELS, LAID_AT,
    LAID_CELLS, LINES_AT, LINES_ENTRIES_AT, LINES_SEGMENTS_AT, ORIGIN_AT, PALETTE_AT, ROOT_AT,
    TREE_AT,
};
use pxl_engine::fill;
use pxl_engine::ground::{self, Resident};
use pxl_engine::lattice::TILE_CELLS;
use pxl_engine::overwrite::{self, Columns, BRANCH};
use pxl_engine::way::{self, tree, Segment, Stations};
use pxl_engine::{value, wrap_idx, Zoom, LEAF_CELL, SHARD_ZOOM};

/// `x` a leaf cell's, as the world names it.
pub fn wrap(x: i32) -> i32 {
    wrap_idx(LEAF_CELL, x)
}

/// The hash made-up content turns on.
pub fn mix(a: u32, c: [i32; 2]) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ 0x51ED_270B;
    for v in c {
        h = (h ^ v as u32).wrapping_mul(0x85EB_CA77).rotate_left(13);
    }
    h ^ h >> 15
}

/// A seeded stream.
pub struct Draw(pub u32);

impl Draw {
    pub fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    /// In `[lo, hi)`.
    pub fn within(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u32) as i32
    }
}

/// The finest zoom asked: what is placed is held no finer (pxl-spec bake `OBJECT_FINEST`).
pub const FINEST: Zoom = LEAF_CELL + 3;

/// Where the eye is, a leaf cell: just east of the world's seam in X, so its finest clip
/// level starts east of it and its coarser ones west, a few cells over the service area's
/// south edge.
pub const EYE: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];

/// The shards served: west of the seam far past what the clip levels reach, a few east of
/// it, and three about X = 0, where X's roots part, from row 40 to 44: an edge of the area
/// within every clip level's reach but the west; and a shard left out in it (`NOTCH`), whose
/// corners only a diagonal leaf cell touches; and the shard at the world's first node, no clip
/// level's reach.
pub fn served(n: [i32; 2]) -> bool {
    (n[0] >= 100 || n[0] < -124 || (-1..=1).contains(&n[0]))
        && (40..44).contains(&n[1])
        && n != NOTCH
        || n == [0, 0]
}

/// The shard left out of the area.
pub const NOTCH: [i32; 2] = [-126, 41];

/// The leaf cells that touch the notch only at a corner, what each is named by a tally.
pub const CORNERS: [(&str, [i32; 2]); 4] = [
    (
        "wall by a diagonal, south-west",
        [(NOTCH[0] << 16) - 1, (NOTCH[1] << 16) - 1],
    ),
    (
        "wall by a diagonal, south-east",
        [(NOTCH[0] + 1) << 16, (NOTCH[1] << 16) - 1],
    ),
    (
        "wall by a diagonal, north-west",
        [(NOTCH[0] << 16) - 1, (NOTCH[1] + 1) << 16],
    ),
    (
        "wall by a diagonal, north-east",
        [(NOTCH[0] + 1) << 16, (NOTCH[1] + 1) << 16],
    ),
];

/// The rows whose nodes wider than a shard lie wholly under the sea: all from here north.
pub const SUNK_FROM: i32 = 42 << 16;

/// The clip levels wanted and not laid out.
pub const UNLAID: [Zoom; 2] = [15, 21];

/// The first brick of the clip level at `cell`.
pub fn level_first(eye: [i32; 2], cell: Zoom) -> [i32; 2] {
    let s = LEAF_CELL - cell;
    let first = clip::origin(cell, [eye[0] >> s, eye[1] >> s]);
    // At some zooms it starts on a block the whole level's: its top entry is all of it. North
    // to the block, so it holds the area's edge and not what lies past it.
    if cell == 16 || cell == 20 {
        [first[0] & !7, (first[1] + 7) & !7]
    } else {
        first
    }
}

/// What a brick of the clip level at `cell` stands over the rest, its own samples': spires a
/// brick wide about its own edges and each finer level's, so a block read at an edge, or read
/// on at a coarser level, moves a bound.
pub fn spire(eye: [i32; 2], cell: Zoom, x: i32, y: i32) -> i32 {
    let s = BRICK_SHIFT + LEAF_CELL - cell;
    let mut most = 0;
    for finer in cell..=LEAF_CELL {
        let f = BRICK_SHIFT + LEAF_CELL - finer;
        let first = level_first(eye, finer);
        let i = wrap_idx(LEAF_CELL, x - (first[0] << f)) >> f;
        let j = (y - (first[1] << f)) >> f;
        let ring = |v: i32| matches!(v, -1 | 0 | 7 | 8);
        if (-1..=8).contains(&i) && (-1..=8).contains(&j) && (ring(i) || ring(j)) {
            let h = mix(31 * cell + finer, [x >> s, y >> s]);
            if h.is_multiple_of(3) {
                // Each level's taller than a coarser's, so what it reads alone shows.
                most = most.max(500 + 120 * (cell - FIRST_CELL) as i32 + (h >> 8) as i32 % 100);
            }
        }
    }
    most
}

/// The ground's face at a leaf column as the clip level at `cell` holds it, in leaf cells.
pub fn elevation(eye: [i32; 2], cell: Zoom, x: i32, y: i32) -> i32 {
    let (fx, fy) = (x as f32, y as f32);
    let hills = 300.0 * libm::sinf(fx * 1.3e-5) + 120.0 * libm::cosf(fy * 3.1e-5);
    let near = 40.0 * libm::sinf(fx * 0.002) + 30.0 * libm::cosf(fy * 0.0031);
    let fine = 8.0 * libm::sinf((fx + fy) * 0.017);
    let sunk = if y >= SUNK_FROM { -1500 } else { 0 };
    (hills + near + fine) as i32 + sunk + spire(eye, cell, x, y)
}

/// A leaf cell's column at `cell` as `g` (`g` the cell's middle).
pub fn leaf_of(cell: Zoom, g: i32) -> i32 {
    let s = LEAF_CELL - cell;
    (g << s) + ((1 << s) >> 1)
}

/// The world as the GPU holds it, read as pxl-shader's `World` reads it.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub clipmap: &'a [[i32; 4]],
    pub ground: &'a [u32],
    pub palette: &'a [u32],
    pub shards: &'a [[u32; 4]],
    pub overwrites: &'a [u32],
    pub pool: &'a [u32],
}

impl Columns for World<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        self.overwrites[slot as usize]
    }
    fn data(&self, slot: u32) -> u32 {
        self.overwrites[self.overwrites.len() / 2 + slot as usize]
    }
}

impl Area for World<'_> {
    fn ends(&self) -> bool {
        true
    }
    fn nine(&self, key: u32) -> u32 {
        self.shards[key as usize / 4][key as usize % 4]
    }
}

impl World<'_> {
    fn row(&self, r: u32) -> clip::Entry {
        clip::Entry::of(self.clipmap[r as usize])
    }
}

impl Presence for World<'_> {
    fn present(&self, cell: Zoom, q: [i32; 2]) -> (u32, bool) {
        let row = self.row((cell - FIRST_CELL) * ENTRIES + clip::entry(q));
        (row.slot(), row.holds(q))
    }
}

impl Bricks for World<'_> {
    fn brick(&self, cell: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.row((cell - FIRST_CELL) * ENTRIES + entry).brick()
    }
    fn joined(&self, cell: Zoom, e: u32) -> [u32; clip::JOINED_WORDS as usize] {
        self.clipmap[(JOINED_AT + (cell - FIRST_CELL) * JOINED_ENTRIES + e) as usize]
            .map(|v| v as u32)
    }
    fn origin(&self, cell: Zoom) -> ([i32; 2], bool) {
        self.row(ORIGIN_AT + cell - FIRST_CELL).brick()
    }
    fn palette_word(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        self.palette[(slot * BRICK_CELLS + i) as usize]
    }
    fn column(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        fill::column_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn level(&self, _: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        fill::level_of(self.ground, slot * BRICK_WORDS, j, at)
    }
    fn laid(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        fill::laid_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn root(&self, _: Zoom, slot: u32) -> u32 {
        fill::root_of(self.ground, slot * BRICK_WORDS)
    }
}

impl Resident for World<'_> {
    type Content = Pixelet;
    fn relief(
        &mut self,
        place: &mut Lookup,
        cells: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (ground::Bounds, bool) {
        let (r, _, here) = clip::bound_over(self, place, cells, (zoom, [c[0], c[1]]));
        (r, here)
    }
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> ground::Cell {
        clip::ground(self, shard, place, zoom, [c[0], c[1]]).0
    }
}

/// A way's middle a descent is sent to: its cells' zoom, its line, and where in leaf cells.
pub type Point = (Zoom, u32, [i32; 3]);

/// A brick's record as a frame hands it: its four palette indices a cell, its tile's
/// samples, its lines. Its ways' middles are added to `ways`.
pub fn handed(
    eye: [i32; 2],
    fresh: &mut [u32],
    cell: Zoom,
    q: [i32; 2],
    lined: bool,
    ways: &mut Vec<Point>,
) {
    fresh.fill(0);
    let s = LEAF_CELL - cell;
    for i in 0..BRICK_CELLS {
        let [x, y] = clip::of_idx(i);
        let g = [
            (q[0] << BRICK_SHIFT) + x as i32,
            (q[1] << BRICK_SHIFT) + y as i32,
        ];
        let leaf = [leaf_of(cell, g[0]), leaf_of(cell, g[1])];
        let shard = [wrap_idx(SHARD_ZOOM, leaf[0] >> 16), leaf[1] >> 16];
        let four = if served(shard) {
            let region = mix(cell, [g[0] >> 3, g[1] >> 3]);
            let base = match region % 8 {
                0 => palette::sand as u32,
                1 => palette::beach as u32,
                2 => palette::water as u32,
                3 => palette::land as u32,
                _ => 5 + (region >> 8) % 30,
            };
            let h = mix(i, g);
            let at = |k: u32| {
                if h >> (8 * k) & 7 == 0 {
                    1 + (h >> (8 * k + 3)) % 60
                } else {
                    base
                }
            };
            [at(0), at(1), at(2), at(3)]
        } else {
            [ground::UNKNOWN; 4]
        };
        fresh[(PALETTE_AT + i) as usize] = ground::word(four);
    }
    // The tile's samples, each about a padded place on the tile's cells.
    let tile = [q[0] >> 3, q[1] >> 3];
    let side = TILE_CELLS as i32;
    let first = [tile[0] * side, tile[1] * side];
    let step = (TILE_CELLS / ground::TILE_ELE_SPAN) as i32;
    for sy in 0..ground::ELE_PADDED_SPAN {
        for sx in 0..ground::ELE_PADDED_SPAN {
            let at = |v: u32, f: i32| {
                ((f + (v as i32 - ground::ELE_PAD as i32) * step + step / 2) as i64) << s
            };
            let e = elevation(eye, cell, at(sx, first[0]) as i32, at(sy, first[1]) as i32);
            let k = ground::ele_idx(sx, sy) as u32;
            fresh[(clip::ELE_AT + k / 2) as usize] |= (e as i16 as u16 as u32) << (16 * (k % 2));
        }
    }
    // The tile's lines, every one in every bin of the brick: a bridge, a bore and an
    // approach, each two segments that continue.
    let leaf_side = (1u32 << s) as f32;
    let per_m = 1.0f32;
    let stations = Stations {
        step: [
            way::INTERVAL_M[0] * per_m / leaf_side,
            way::INTERVAL_M[1] * per_m / leaf_side,
        ],
        phase: [[0.0; 2], [3.0, 5.0]],
        leaf: leaf_side,
    };
    fresh[LINES_AT as usize..(LINES_AT + way::HEADER_WORDS) as usize]
        .copy_from_slice(&way::header(per_m, &stations));
    if !lined {
        return;
    }
    let mut segments = Vec::new();
    for line in 0..5u32 {
        let h = mix(line + 100 * cell, tile);
        // Across the brick's own quarter of the tile, so its bricks hold it.
        // The high bridge apart from the rest, its nodes over no other's.
        let beside = if line == 3 { 22 } else { 0 };
        let center = [(q[0] & 7) * 64 + 32, (q[1] & 7) * 64 + 32 + beside];
        let off = |r: u32| ((r % 8) as i32 - 4) * 8;
        let a = [(center[0] - 40) * 8, center[1] * 8 + off(h)];
        let b = [(center[0] + 40) * 8, center[1] * 8 + off(h >> 8)];
        let m = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2 + off(h >> 16)];
        // A bridge, a bore and an approach; a bridge high over the ground; a bore through
        // the surface, its air over the ground's top, tall enough to fill a coarse cell.
        let form = [
            way::BRIDGE,
            way::TUNNEL,
            way::APPROACH,
            way::BRIDGE,
            way::TUNNEL,
        ][line as usize];
        let lift = [10, -25, 2, 60, -2][line as usize];
        let top_at = |p: [i32; 2]| {
            let leaf = |v: i32, f: i32| (f + v / 8) << s;
            (elevation(eye, cell, leaf(p[0], first[0]), leaf(p[1], first[1])) + lift) as i16
        };
        let width = 16 + (h >> 24) % 48;
        let kinds = value::TRANSPORTS[1] - value::TRANSPORTS[0] + 1;
        let kind = value::TRANSPORTS[0] + (h >> 4) % kinds;
        let clear = match line {
            1 => 6,
            4 => (1u32 << s).clamp(6, 250),
            _ => 0,
        };
        let point = |p: [i32; 2]| [p[0] as i16, p[1] as i16];
        for (k, (from, to)) in [(a, m), (m, b)].into_iter().enumerate() {
            let flags = way::flags(form, k == 0, k == 1, k == 1);
            segments.push(Segment::words(
                [point(from), point(to)],
                [top_at(from), top_at(to)],
                [width as u8, kind as u8, flags, clear as u8],
            ));
        }
        let mid = [first[0] + m[0] / 8, first[1] + m[1] / 8];
        ways.push((cell, line, [mid[0] << s, mid[1] << s, top_at(m) as i32]));
    }
    let n = segments.len() as u32;
    for b in 0..clip::WAYS_BINS {
        fresh[(LINES_AT + way::HEADER_WORDS + b) as usize] = n << 16;
    }
    for e in 0..n {
        fresh[(LINES_AT + LINES_ENTRIES_AT + e / 2) as usize] |= e << (16 * (e % 2));
    }
    for (k, w) in segments.iter().enumerate() {
        let at = (LINES_AT + LINES_SEGMENTS_AT) as usize + k * way::SEGMENT_WORDS;
        fresh[at..at + way::SEGMENT_WORDS].copy_from_slice(w);
    }
}

/// What is placed as the store lays it: a slot's child pointer, then its data; the
/// root's child group first; each branch its children's fold.
pub struct Placing {
    first: Vec<u32>,
    data: Vec<u32>,
}

impl Placing {
    fn new() -> Placing {
        Placing {
            first: vec![0; 8],
            data: vec![0; 8],
        }
    }

    /// The leaf `word` at the node `c` of `zoom`, unless a coarser leaf covers it.
    fn put(&mut self, zoom: Zoom, c: [i32; 3], word: u32) {
        let p = overwrite::path(zoom, c);
        let idx = |d: Zoom| {
            let v = [p[0] >> d, p[1] >> d, p[2] >> d];
            (v[0] & 1 | (v[1] & 1) << 1 | (v[2] & 1) << 2) as usize
        };
        let mut slot = idx(zoom);
        for z in 1..=zoom {
            if self.first[slot] == 0 {
                if self.data[slot] != 0 {
                    return;
                }
                let base = self.first.len() as u32;
                self.first.resize(base as usize + 8, 0);
                self.data.resize(base as usize + 8, 0);
                self.first[slot] = BRANCH | base;
            }
            slot = (self.first[slot] & !BRANCH) as usize + idx(zoom - z);
        }
        self.first[slot] = 0;
        self.data[slot] = word;
    }

    /// The fold of the branch at `slot`, a node of `zoom`.
    fn fold(&mut self, slot: usize, zoom: Zoom) -> u32 {
        if self.first[slot] == 0 {
            return overwrite::share(self.data[slot]);
        }
        let base = (self.first[slot] & !BRANCH) as usize;
        let mut children = [0; 8];
        for (k, c) in children.iter_mut().enumerate() {
            *c = self.fold(base + k, zoom + 1);
        }
        self.data[slot] = overwrite::fold(children, zoom);
        self.data[slot]
    }

    fn words(mut self) -> Vec<u32> {
        for k in 0..8 {
            self.fold(k, 0);
        }
        self.first.extend(self.data);
        self.first
    }
}

/// The world built, and what lies where a test sends what it asks.
pub struct Built {
    pub clipmap: Vec<[i32; 4]>,
    pub ground: Vec<u32>,
    pub palette: Vec<u32>,
    pub shards: Vec<[u32; 4]>,
    pub overwrites: Vec<u32>,
    pub pool: Vec<u32>,
    /// The ways' middles.
    pub way_points: Vec<Point>,
    /// What was placed, its zoom and node.
    pub placed: Vec<(Zoom, [i32; 3])>,
    /// Cells what is placed folds in; lone leaves of the finest; points over high bridges;
    /// some of a cell placed where no clip level holds the ground under it.
    pub folds: Vec<[i32; 3]>,
    pub lone: Vec<[i32; 3]>,
    pub over_bridges: Vec<[i32; 3]>,
    pub unheld: Vec<[i32; 3]>,
    /// The stream it was built by, going on.
    pub draw: Draw,
}

impl Built {
    pub fn world(&self) -> World<'_> {
        World {
            clipmap: &self.clipmap,
            ground: &self.ground,
            palette: &self.palette,
            shards: &self.shards,
            overwrites: &self.overwrites,
            pool: &self.pool,
        }
    }

    /// A ground's top, off whichever clip level holds it, what is placed read from none.
    pub fn top_at(&self, g: [i32; 2]) -> i32 {
        let (_, column, _, _) = clip::cell(
            &self.world(),
            Lookup::NONE,
            LEAF_CELL,
            [wrap_idx(LEAF_CELL, g[0]), g[1]],
        );
        clip::column_top(column)
    }
}

/// The eye's clip levels' entries in `clipmap` folded a level at a time over its entry table
/// (`clip::fold`), and some left as a clip level moved along one axis left them, not yet
/// folded again.
pub fn fold(
    clipmap: &mut [[i32; 4]],
    ground: &[u32],
    palette: &[u32],
    shards: &[[u32; 4]],
    pool: &[u32],
) {
    for l in 1..=JOINED_LEVELS {
        for cell in FIRST_CELL..=LEAF_CELL {
            let entries: Vec<_> = {
                let w = World {
                    clipmap,
                    ground,
                    palette,
                    shards,
                    overwrites: &[0; 16],
                    pool,
                };
                (0..(ACROSS >> l) * (ACROSS >> l))
                    .map(|b| clip::fold(&w, cell, l, b))
                    .collect()
            };
            for (b, e) in entries.into_iter().enumerate() {
                let at =
                    JOINED_AT + (cell - FIRST_CELL) * JOINED_ENTRIES + clip::joined_idx(l, [0, 0]);
                clipmap[at as usize + b] = e.map(|v| v as i32);
            }
        }
    }
    for e in 0..(JOINED_ENTRIES * EYE_CLIP_LEVELS) as usize {
        let h = mix(e as u32, [7, 11]);
        if h.is_multiple_of(5) {
            clipmap[JOINED_AT as usize + e][(h >> 8 & 1) as usize] ^= 1;
        }
    }
}

/// The service area's index, a cell's nine by its key (`served`).
pub fn shards() -> Vec<[u32; 4]> {
    let mut shards = vec![[0u32; 4]; (area::SHARD_CELLS / 4) as usize];
    for y in 0..1 << SHARD_ZOOM {
        for x in 0..1 << SHARD_ZOOM {
            let key = area::shard_key([x, y]) as usize;
            shards[key / 4][key % 4] = area::nine(served, [x, y]);
        }
    }
    shards
}

/// The world about an eye at `eye`, seeded the one way.
pub fn build(eye: [i32; 2]) -> Built {
    let shards = shards();

    // The clip levels and their bricks, each filled as a frame's fill does; some wanted
    // and not yet held.
    let rows = (JOINED_AT + EYE_CLIP_LEVELS * JOINED_ENTRIES) as usize;
    let mut clipmap = vec![[0i32; 4]; rows];
    let mut ground_words: Vec<u32> = Vec::new();
    let mut palette_words: Vec<u32> = Vec::new();
    let mut pool = vec![0u32; 1 << 23];
    let mut next_group = 1u32;
    let (mut fresh, mut lane, mut scratch) = (
        vec![0u32; HANDED_WORDS as usize],
        vec![0u32; BRICK_WORDS as usize],
        vec![0u32; TREE_AT as usize],
    );
    let mut way_points = Vec::new();
    let (mut held_bricks, mut absent_bricks, mut way_bricks) = (0u32, 0u32, 0u32);
    for cell in FIRST_CELL..=LEAF_CELL {
        let first = level_first(eye, cell);
        let laid = !UNLAID.contains(&cell) as i32;
        clipmap[(ORIGIN_AT + cell - FIRST_CELL) as usize] = [first[0], first[1], laid, 0];
        for k in 0..ENTRIES {
            let q = [
                wrap_idx(clip::bricks_zoom(cell), first[0] + (k % ACROSS) as i32),
                first[1] + (k / ACROSS) as i32,
            ];
            let row = ((cell - FIRST_CELL) * ENTRIES + clip::entry(q)) as usize;
            clipmap[row] = [q[0], q[1], 0, 0];
            let h = mix(cell, q);
            if h.is_multiple_of(7) {
                absent_bricks += 1;
                continue;
            }
            let lined = !(h >> 8).is_multiple_of(3);
            handed(eye, &mut fresh, cell, q, lined, &mut way_points);
            lane.fill(0);
            let place = [(q[0] & 7) as u32, (q[1] & 7) as u32];
            let first_cell = [wrap_idx(cell, q[0] << BRICK_SHIFT), q[1] << BRICK_SHIFT];
            let palette_at = PALETTE_AT as usize;
            palette_words.extend_from_slice(&fresh[palette_at..palette_at + BRICK_CELLS as usize]);
            fill::brick(&mut fresh, &mut lane, &mut scratch, place, cell, first_cell);
            let [taken, _, root] = fill::tree(&lane, &scratch, cell, &mut pool, next_group);
            if taken != u32::MAX {
                next_group += taken;
                lane[ROOT_AT as usize] = root;
                way_bricks += (root != 0) as u32;
            }
            let slot = held_bricks;
            ground_words.extend_from_slice(&lane);
            clipmap[row][2] = slot as i32 + 1;
            held_bricks += 1;
        }
    }
    assert!(ground_words.len() as u32 / BRICK_WORDS == held_bricks);
    // Some of each brick's columns no ground, its cells laid void, under ways and not: what the
    // ways stand on there, or nothing.
    for slot in 0..held_bricks {
        for i in 0..BRICK_CELLS {
            if mix(slot + 1000, [i as i32, 3]).is_multiple_of(29) {
                let w = (slot * BRICK_WORDS + LAID_AT + i / LAID_CELLS) as usize;
                ground_words[w] &= !(0xff << (8 * (i % LAID_CELLS)));
            }
        }
    }
    // Slot 0 a brick with a tree a node of any brick's Z may read: a place held by none, its
    // slot 0, reads no slot.
    let words = BRICK_WORDS as usize;
    let treed = (0..held_bricks as usize)
        .find(|&s| {
            let root = ground_words[s * words + ROOT_AT as usize];
            let (first, z0) = tree::root_slot(root);
            root != 0
                && [-z0, -1 - z0].iter().any(|&k| {
                    (0..8).contains(&k) && tree::Read(&pool).first_child(first + k as u32) != 0
                })
        })
        .expect("a brick whose tree lies about sea level");
    if treed != 0 {
        let (zero, rest) = ground_words.split_at_mut(treed * words);
        zero[..words].swap_with_slice(&mut rest[..words]);
        let cells = BRICK_CELLS as usize;
        let (zero, rest) = palette_words.split_at_mut(treed * cells);
        zero[..cells].swap_with_slice(&mut rest[..cells]);
        for row in &mut clipmap[..ORIGIN_AT as usize] {
            if row[2] == 1 {
                row[2] = treed as i32 + 1;
            } else if row[2] == treed as i32 + 1 {
                row[2] = 1;
            }
        }
    }
    eprintln!(
        "{held_bricks} bricks held, {absent_bricks} absent, {way_bricks} with ways over {next_group} child groups"
    );
    let overwrites_none = vec![0u32; 16];
    fold(&mut clipmap, &ground_words, &palette_words, &shards, &pool);

    // A ground's top, off whichever clip level holds it.
    let top_at = |g: [i32; 2]| {
        let w = World {
            clipmap: &clipmap,
            ground: &ground_words,
            palette: &palette_words,
            shards: &shards,
            overwrites: &overwrites_none,
            pool: &pool,
        };
        let (_, column, _, _) = clip::cell(
            &w,
            Lookup::NONE,
            LEAF_CELL,
            [wrap_idx(LEAF_CELL, g[0]), g[1]],
        );
        clip::column_top(column)
    };

    // What is placed about the eye, and further off, each kind a leaf of its own.
    let mut draw = Draw(0x1234_5679);
    let mut placing = Placing::new();
    let mut placed: Vec<(Zoom, [i32; 3])> = Vec::new();
    let mut folds: Vec<[i32; 3]> = Vec::new();
    for i in 0..2400u32 {
        let reach = if i % 4 == 0 { 3000 } else { 240 };
        let g = [
            eye[0] + draw.within(-reach, reach),
            (eye[1] + draw.within(-reach, reach)).max((40 << 16) + 2),
        ];
        let t = top_at(g);
        let leaf = |zoom: Zoom, dz: i32| {
            let c = |v: i32| {
                if zoom >= LEAF_CELL {
                    v << (zoom - LEAF_CELL)
                } else {
                    v >> (LEAF_CELL - zoom)
                }
            };
            (zoom, [wrap_idx(zoom, c(g[0])), c(g[1]), c(t + dz)])
        };
        let mut lay = |(zoom, c): (Zoom, [i32; 3]), word: u32| {
            placing.put(zoom, c, word);
            placed.push((zoom, c));
        };
        match i % 12 {
            // Finer than a cell: a cell's eight, a byte each, all of it, the half of it
            // low, air over a byte, or decor's alone, which fills none.
            8..=11 => {
                let (z, c) = leaf(25, 0);
                let at = [c[0] & !1, c[1] & !1, c[2] & !1];
                for k in 0..8u32 {
                    let word = match i % 12 {
                        8 => overwrite::leaf_word(5 + k * 3 + i % 7, 0),
                        9 if k < 4 => overwrite::leaf_word(6 + k * 2, 0),
                        10 if k < 4 => overwrite::leaf_word(40 + k, 0),
                        10 if k % 2 == 0 => overwrite::leaf_word(value::AIR, 0),
                        11 => overwrite::leaf_word(0x1500 + k, i),
                        _ => continue,
                    };
                    let d = [(k & 1) as i32, (k >> 1 & 1) as i32, (k >> 2) as i32];
                    lay((z, [at[0] + d[0], at[1] + d[1], at[2] + d[2]]), word);
                }
                folds.push([g[0], g[1], c[2] >> 1]);
            }
            // An object's color, a ground's palette entry, its handle over it.
            0 => lay(
                leaf(LEAF_CELL, draw.within(0, 4)),
                overwrite::leaf_word(
                    match i / 12 % 3 {
                        0 => value::OBJECTS[1],
                        1 => value::OBJECTS[0],
                        _ => 0x101 + i % 40,
                    },
                    i,
                ),
            ),
            1 => lay(
                leaf(LEAF_CELL, 0),
                overwrite::leaf_word(
                    match i / 12 % 3 {
                        0 => value::GROUNDS[1],
                        1 => value::GROUNDS[0],
                        _ => 5 + i % 30,
                    },
                    i,
                ),
            ),
            // Air, where a cube is cut out of the ground.
            2 => lay(
                leaf(LEAF_CELL, -draw.within(1, 5)),
                overwrite::leaf_word(value::AIR, 0),
            ),
            // A leaf over several cells.
            3 => lay(
                leaf(22, draw.within(0, 9)),
                overwrite::leaf_word(0x200 + i % 40, 0),
            ),
            // Finer than a cell: some of its cube.
            4 => {
                let (z, c) = leaf(FINEST - (i / 12 % 2), 0);
                for k in 0..1 + draw.next() % 40 {
                    let d = |b: u32| ((k >> b & 3) as i32) & 3;
                    let word = overwrite::leaf_word(0x120 + i % 5, 0);
                    lay((z, [c[0] + d(0), c[1] + d(2), c[2] + d(4)]), word);
                }
            }
            // A fine leaf a byte's and air beside it.
            5 => {
                let (z, c) = leaf(25, 1);
                lay((z, c), overwrite::leaf_word(20 + i % 9, 0));
                lay(
                    (z, [c[0] + 1, c[1], c[2]]),
                    overwrite::leaf_word(value::AIR, 0),
                );
                lay((z, [c[0], c[1] + 1, c[2]]), overwrite::leaf_word(0x140, 0));
            }
            // A value that is no voxel: decor's.
            6 => lay(leaf(LEAF_CELL, 2), overwrite::leaf_word(0x1234, i)),
            // A large leaf further off.
            _ => lay(
                leaf(18 + i % 5, draw.within(-40, 60)),
                overwrite::leaf_word(0x180, 0),
            ),
        }
    }
    // A lone leaf of the finest in its node of zoom 21, nothing else near: a fold there has
    // a kind and no share.
    let mut lone = Vec::new();
    for k in 0..300 {
        let g = [(-127 << 16) + 16 * k, (43 << 16) + 400 + 16 * (k % 40)];
        let p = [g[0], g[1], top_at(g) + 30];
        let at = [wrap_idx(FINEST, p[0] << 3), p[1] << 3, p[2] << 3];
        let val = if k % 2 == 0 { 0x150 } else { value::AIR };
        placing.put(FINEST, at, overwrite::leaf_word(val, 0));
        lone.push(p);
    }
    // In the upper half of a node over a high bridge at the leaves, the ground under it:
    // its box and the bridge's both short along z.
    let mut over_bridges = Vec::new();
    for &(cell, line, p) in &way_points {
        let side = 16;
        if cell == LEAF_CELL && line == 3 && (3..8).contains(&p[2].rem_euclid(side)) {
            let z = p[2].div_euclid(side) * side + side / 2 + side / 4;
            for at in [[p[0], p[1], z], [p[0] + 3, p[1] - 3, z + 1]] {
                let c = [wrap_idx(LEAF_CELL, at[0]), at[1], at[2]];
                placing.put(LEAF_CELL, c, overwrite::leaf_word(0x170, 0));
            }
            over_bridges.push(p);
        }
    }
    // Some of a cell placed where no clip level holds the ground under it.
    let mut unheld = Vec::new();
    for k in 0..300 {
        let leaf = [
            (110 << 16) + draw.within(-4000, 4000),
            (42 << 16) + draw.within(-4000, 4000),
            draw.within(-100, 100),
        ];
        let at = leaf.map(|v| v << 1);
        placing.put(
            25,
            at,
            overwrite::leaf_word(if k % 2 == 0 { 9 } else { 0x130 }, 0),
        );
        unheld.push(leaf);
    }
    let overwrites = placing.words();
    Built {
        clipmap,
        ground: ground_words,
        palette: palette_words,
        shards,
        overwrites,
        pool,
        way_points,
        placed,
        folds,
        lone,
        over_bridges,
        unheld,
        draw,
    }
}
