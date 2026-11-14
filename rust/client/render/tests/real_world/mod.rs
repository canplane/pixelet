//! A real world's buffers, made on the CPU as the fill and the store lay them, for the tests of
//! the world's one query by hand and of the walks over it. The world holds clip levels and
//! their bricks, the service area's index, what is placed and the ways' store. `Asking` is the
//! query a walk on the CPU asks of these buffers, and `WorldBuffer` holds the same buffers on the
//! device.

#![allow(dead_code, reason = "each test reads its own of it")]

use pxl_content::Pixelet;
use pxl_engine::answer::In;
use pxl_engine::cast::Query;
use pxl_engine::clipmap as clip;
use pxl_engine::clipmap::{
    BrickSource, Lookup, ACROSS, BRICK_WORDS, ENTRY_CNT, EYE_CLIP_LEVEL_CNT, FIRST_CELL_ZOOM,
    JOINED_AT, JOINED_ENTRY_CNT, JOINED_MIP_CNT, ORIGIN_AT, OWN_AT, OWN_CELL_CNT, ROOT_AT,
    SPANS_WORDS,
};
use pxl_engine::fill;
use pxl_engine::ground::{self, Resident};
use pxl_engine::object::{self, Table, BRANCH};
use pxl_engine::world::{self, Inheritance};
use pxl_spec::asset::cube::{Ground, Sentinel};
use pxl_spec::asset::decor::DECORS;
use pxl_spec::brick;
use pxl_spec::brick::{BRICK_CELL_CNT, BRICK_SHIFT, LINES_BINS_AT, LINES_ENTRIES_AT};
use pxl_spec::ground::{ele_idx, word, ELE_PAD, ELE_PADDED_SPAN, TILE_ELE_SPAN};
use pxl_spec::space::coord::adapter::epsg3857::cells_of_m;
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::world::wrap_idx;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM, SHARD_ZOOM};
use pxl_spec::space::layer::area;
use pxl_spec::space::layer::area::{Area, Shard};
use pxl_spec::space::layer::lattice::TILE_CELLS;
use pxl_spec::value;
use pxl_spec::way;
use pxl_spec::way::{Segment, StationGrid};

/// The leaf cell index `x`, wrapped round X as the world names it.
pub fn wrap(x: i32) -> i32 {
    wrap_idx(LEAF_CELL_ZOOM, x)
}

/// The hash the made-up content is drawn from.
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
    /// The next draw, within `[lo, hi)`.
    pub fn within(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u32) as i32
    }
}

/// The finest zoom a test asks. What is placed is held no finer (pxl-spec bake
/// `OBJECT_FINEST`).
pub const FINEST: Zoom = LEAF_CELL_ZOOM + 3;

/// Where the eye stands, in leaf cells. It is just east of the world's seam in X, so its finest
/// clip level starts east of the seam and its coarser levels start west of it. It is a few
/// cells north of the service area's south edge.
pub const EYE: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];

/// Whether shard `n` is served. The served shards lie in rows 40 to 43: west of the seam from
/// X = 100, far past what the clip levels reach; a few east of the seam; and three about
/// X = 0, where X's roots part. Every edge of the area but the west lies within every clip
/// level's reach. One shard inside is left out (`NOTCH`), and some leaf cells touch it only at
/// a corner (`CORNERS`). The shard at the world's first node is served too, out of every clip
/// level's reach.
pub fn served(n: [i32; 2]) -> bool {
    (n[0] >= 100 || n[0] < -124 || (-1..=1).contains(&n[0]))
        && (40..44).contains(&n[1])
        && n != NOTCH
        || n == [0, 0]
}

/// The shard left out of the area.
pub const NOTCH: [i32; 2] = [-126, 41];

/// The leaf cells that touch the notch only at a corner, each with the name a tally counts it
/// by.
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

/// The first row of the land sunk under the sea. From this row north, nodes wider than a shard
/// lie wholly under the sea.
pub const SUNK_FROM: i32 = 42 << 16;

/// The clip levels the world wants but has not laid out.
pub const UNLAID: [Zoom; 2] = [15, 21];

/// The first brick of the clip level at `cell_zoom`.
pub fn level_first(eye: [i32; 2], cell_zoom: Zoom) -> [i32; 2] {
    let s = LEAF_CELL_ZOOM - cell_zoom;
    let first = clip::origin(cell_zoom, [eye[0] >> s, eye[1] >> s]);
    // At zooms 16 and 20 the level starts on a block as wide as the whole level, so its top
    // entry covers all of it. The start rounds north to the block, so the level holds the
    // area's south edge and not what lies past it.
    if cell_zoom == 16 || cell_zoom == 20 {
        [first[0] & !7, (first[1] + 7) & !7]
    } else {
        first
    }
}

/// How far the samples of a brick of the clip level at `cell_zoom` stand over the rest of the
/// ground. Spires a brick wide stand about the edges of this level and of each finer level, so
/// a block read at an edge, or read at a coarser level, moves a bound.
pub fn spire(eye: [i32; 2], cell_zoom: Zoom, x: i32, y: i32) -> i32 {
    let s = BRICK_SHIFT + LEAF_CELL_ZOOM - cell_zoom;
    let mut most = 0;
    for finer in cell_zoom..=LEAF_CELL_ZOOM {
        let f = BRICK_SHIFT + LEAF_CELL_ZOOM - finer;
        let first = level_first(eye, finer);
        let i = wrap_idx(LEAF_CELL_ZOOM, x - (first[0] << f)) >> f;
        let j = (y - (first[1] << f)) >> f;
        let ring = |v: i32| matches!(v, -1 | 0 | 7 | 8);
        if (-1..=8).contains(&i) && (-1..=8).contains(&j) && (ring(i) || ring(j)) {
            let h = mix(31 * cell_zoom + finer, [x >> s, y >> s]);
            if h.is_multiple_of(3) {
                // Each level's spires stand taller than a coarser level's, so a read that only
                // this level answers shows in the bound.
                most = most
                    .max(500 + 120 * (cell_zoom - FIRST_CELL_ZOOM) as i32 + (h >> 8) as i32 % 100);
            }
        }
    }
    most
}

/// The ground's face at a leaf column as the clip level at `cell_zoom` holds it, in leaf cells.
pub fn ele(eye: [i32; 2], cell_zoom: Zoom, x: i32, y: i32) -> i32 {
    let (fx, fy) = (x as f32, y as f32);
    let hills = 300.0 * libm::sinf(fx * 1.3e-5) + 120.0 * libm::cosf(fy * 3.1e-5);
    let near = 40.0 * libm::sinf(fx * 0.002) + 30.0 * libm::cosf(fy * 0.0031);
    let fine = 8.0 * libm::sinf((fx + fy) * 0.017);
    let sunk = if y >= SUNK_FROM { -1500 } else { 0 };
    (hills + near + fine) as i32 + sunk + spire(eye, cell_zoom, x, y)
}

/// The leaf column at the middle of column `g` of the clip level at `cell_zoom`.
pub fn leaf_of(cell_zoom: Zoom, g: i32) -> i32 {
    let s = LEAF_CELL_ZOOM - cell_zoom;
    (g << s) + ((1 << s) >> 1)
}

/// The world as the GPU holds it, read as clip.wgsl and world.wgsl read it.
#[derive(Clone, Copy)]
pub struct World<'a> {
    pub clipmap: &'a [[i32; 4]],
    pub ground: &'a [u32],
    pub folds: &'a [u32],
    pub shards: &'a [[u32; 4]],
    pub objects: &'a [u32],
    pub pool: &'a [u32],
}

impl Table for World<'_> {
    fn first_child(&self, slot: u32) -> u32 {
        self.objects[slot as usize]
    }
    fn data(&self, slot: u32) -> u32 {
        self.objects[self.objects.len() / 2 + slot as usize]
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

impl BrickSource for World<'_> {
    fn present(&self, cell_zoom: Zoom, q: [i32; 2]) -> (u32, bool) {
        let row = self.row((cell_zoom - FIRST_CELL_ZOOM) * ENTRY_CNT + clip::entry(q));
        (row.slot(), row.holds(q))
    }
    fn brick(&self, cell_zoom: Zoom, entry: u32) -> ([i32; 2], bool) {
        self.row((cell_zoom - FIRST_CELL_ZOOM) * ENTRY_CNT + entry)
            .brick()
    }
    fn joined(&self, cell_zoom: Zoom, e: u32) -> [u32; clip::JOINED_WORDS as usize] {
        self.clipmap[(JOINED_AT + (cell_zoom - FIRST_CELL_ZOOM) * JOINED_ENTRY_CNT + e) as usize]
            .map(|v| v as u32)
    }
    fn origin(&self, cell_zoom: Zoom) -> ([i32; 2], bool) {
        self.row(ORIGIN_AT + cell_zoom - FIRST_CELL_ZOOM).brick()
    }
    fn fold_at(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        self.folds[(slot * BRICK_CELL_CNT + i) as usize]
    }
    fn column(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        fill::column_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn bound_mip(&self, _: Zoom, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        fill::mip_of(self.ground, slot * BRICK_WORDS, j, at)
    }
    fn own(&self, _: Zoom, slot: u32, i: u32) -> u32 {
        fill::own_of(self.ground, slot * BRICK_WORDS, i)
    }
    fn root(&self, _: Zoom, slot: u32) -> u32 {
        fill::root_of(self.ground, slot * BRICK_WORDS)
    }
}

impl Resident for World<'_> {
    type Content = Pixelet;
    type Place = Lookup;
    const NOWHERE: Lookup = Lookup::NONE;
    fn relief(
        &mut self,
        place: &mut Lookup,
        cell_range: [Zoom; 2],
        (zoom, c): (Zoom, [i32; 3]),
    ) -> (ground::Extent, bool) {
        let (r, _, here) = clip::bound_over(self, place, cell_range, (zoom, [c[0], c[1]]));
        (r, here)
    }
    fn cell(&mut self, shard: Shard, place: Lookup, zoom: Zoom, c: [i32; 3]) -> ground::Cell {
        clip::ground(self, shard, place, zoom, [c[0], c[1]]).0
    }
}

/// The middle of a way, where a descent is sent: the zoom of the way's cells, its line, and the
/// place in leaf cells.
pub type Point = (Zoom, u32, [i32; 3]);

/// What a brick is filled from (pxl-engine `fill::Site`): four palette indices for each cell,
/// the samples of its tile, and its lines.
pub struct Source {
    pub four: Vec<u32>,
    pub ele: Vec<i16>,
    pub lines: Vec<u32>,
}

/// What the core hands the fill for brick `q` of the clip level at `cell_zoom`. The brick has
/// lines only where `lined` is set, and the middles of its ways are added to `ways`.
pub fn source(
    eye: [i32; 2],
    cell_zoom: Zoom,
    q: [i32; 2],
    lined: bool,
    ways: &mut Vec<Point>,
) -> Source {
    let span = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
    let mut source = Source {
        four: vec![0; BRICK_CELL_CNT as usize],
        ele: vec![0; span],
        lines: vec![0; brick::lines_words(0, 0) as usize],
    };
    let s = LEAF_CELL_ZOOM - cell_zoom;
    for i in 0..BRICK_CELL_CNT {
        let [x, y] = brick::of_idx(i);
        let g = [
            (q[0] << BRICK_SHIFT) + x as i32,
            (q[1] << BRICK_SHIFT) + y as i32,
        ];
        let leaf = [leaf_of(cell_zoom, g[0]), leaf_of(cell_zoom, g[1])];
        let shard = [wrap_idx(SHARD_ZOOM, leaf[0] >> 16), leaf[1] >> 16];
        let four = if served(shard) {
            let region = mix(cell_zoom, [g[0] >> 3, g[1] >> 3]);
            let base = match region % 8 {
                0 => Ground::Sand as u32,
                1 => Ground::Beach as u32,
                2 => Ground::Water as u32,
                3 => Ground::Land as u32,
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
            [Sentinel::Unknown as u32; 4]
        };
        source.four[i as usize] = word(four);
    }
    // The tile's elevation samples, each at the middle of its step on the padded grid over the
    // tile's cells.
    let tile = [q[0] >> 3, q[1] >> 3];
    let side = TILE_CELLS as i32;
    let first = [tile[0] * side, tile[1] * side];
    let step = (TILE_CELLS / TILE_ELE_SPAN) as i32;
    for sy in 0..ELE_PADDED_SPAN {
        for sx in 0..ELE_PADDED_SPAN {
            let at =
                |v: u32, f: i32| ((f + (v as i32 - ELE_PAD as i32) * step + step / 2) as i64) << s;
            let e = ele(
                eye,
                cell_zoom,
                at(sx, first[0]) as i32,
                at(sy, first[1]) as i32,
            );
            source.ele[ele_idx(sx, sy)] = e as i16;
        }
    }
    // The tile's lines, each listed in every bin of the brick. They are bridges, bores and an
    // approach, each of two segments that continue one into the other.
    let per_m = 1.0f32;
    let stations = StationGrid {
        step: way::INTERVAL_M.map(|m| cells_of_m(m, per_m)),
        phase: [[0; 2], [3 << s, 5 << s]],
        leaf: 1 << s,
    };
    source.lines[..way::HDR_WORDS as usize].copy_from_slice(&way::hdr(per_m, &stations));
    if !lined {
        return source;
    }
    let mut segments = Vec::new();
    for line in 0..5u32 {
        let h = mix(line + 100 * cell_zoom, tile);
        // Each line crosses the brick's own part of the tile, so the brick holds it. The high
        // bridge, line 3, lies apart from the rest, so none of its nodes lie over another's.
        let beside = if line == 3 { 22 } else { 0 };
        let center = [(q[0] & 7) * 64 + 32, (q[1] & 7) * 64 + 32 + beside];
        let off = |r: u32| ((r % 8) as i32 - 4) * 8;
        let a = [(center[0] - 40) * 8, center[1] * 8 + off(h)];
        let b = [(center[0] + 40) * 8, center[1] * 8 + off(h >> 8)];
        let m = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2 + off(h >> 16)];
        // Lines 0 to 2 are a bridge, a bore and an approach. Line 3 is a bridge high over the
        // ground. Line 4 is a bore through the surface, whose air rises over the ground's top
        // and is tall enough to fill a coarse cell.
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
            (ele(eye, cell_zoom, leaf(p[0], first[0]), leaf(p[1], first[1])) + lift) as i16
        };
        let width = 16 + (h >> 24) % 48;
        let kinds = value::TRANSPORT[1] - value::TRANSPORT[0] + 1;
        let asset_id = value::TRANSPORT[0] + (h >> 4) % kinds;
        let clearance = match line {
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
                [width as u8, asset_id as u8, flags, clearance as u8],
            ));
        }
        let mid = [first[0] + m[0] / 8, first[1] + m[1] / 8];
        ways.push((
            cell_zoom,
            line,
            [mid[0] << s, mid[1] << s, top_at(m) as i32],
        ));
    }
    // Every bin holds every segment.
    let n = segments.len() as u32;
    let entries = brick::WAYS_BIN_CNT * n;
    let mut lines = vec![0; brick::lines_words(entries, n) as usize];
    lines[..way::HDR_WORDS as usize].copy_from_slice(&source.lines[..way::HDR_WORDS as usize]);
    for b in 0..brick::WAYS_BIN_CNT {
        lines[(LINES_BINS_AT + b) as usize] = b * n;
    }
    lines[(LINES_BINS_AT + brick::WAYS_BIN_CNT) as usize] = entries;
    for e in 0..entries {
        lines[(LINES_ENTRIES_AT + e) as usize] = e % n;
    }
    for (k, w) in segments.iter().enumerate() {
        let at = (LINES_ENTRIES_AT + entries) as usize + k * way::SEGMENT_WORDS;
        lines[at..at + way::SEGMENT_WORDS].copy_from_slice(w);
    }
    source.lines = lines;
    source
}

/// What is placed, as the store lays it. The words are every slot's child pointer, then every
/// slot's data. The root's child group comes first, and each branch's data is the fold of its
/// children.
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

    /// Puts the leaf `word` at the node `c` of `zoom`, unless a coarser leaf covers it.
    fn put(&mut self, zoom: Zoom, c: [i32; 3], word: u32) {
        let p = object::path(zoom, c);
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

    /// Folds the branch at `slot`, a node of `zoom`, into its data, and returns the fold.
    fn fold(&mut self, slot: usize, zoom: Zoom) -> u32 {
        if self.first[slot] == 0 {
            return object::share(self.data[slot]);
        }
        let base = (self.first[slot] & !BRANCH) as usize;
        let mut children = [0; 8];
        for (k, c) in children.iter_mut().enumerate() {
            *c = self.fold(base + k, zoom + 1);
        }
        self.data[slot] = object::fold(children, zoom);
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

/// The world built, and the places where the tests send their asks.
pub struct Built {
    pub clipmap: Vec<[i32; 4]>,
    pub ground: Vec<u32>,
    pub folds: Vec<u32>,
    pub shards: Vec<[u32; 4]>,
    pub objects: Vec<u32>,
    pub pool: Vec<u32>,
    pub way_points: Vec<Point>,
    /// What was placed, each as its zoom and node.
    pub placed: Vec<(Zoom, [i32; 3])>,
    /// `folded` holds cells that what is placed folds into, and `lone` holds lone leaves of the
    /// finest zoom. `over_bridges` holds points over high bridges, and `groundless` holds cells
    /// partly placed where no clip level holds the ground under them.
    pub folded: Vec<[i32; 3]>,
    pub lone: Vec<[i32; 3]>,
    pub over_bridges: Vec<[i32; 3]>,
    pub groundless: Vec<[i32; 3]>,
    /// The decor placed, each as one leaf cell.
    pub decors: Vec<[i32; 3]>,
    /// The stream the world was built by, left where the build stopped drawing from it.
    pub draw: Draw,
}

impl Built {
    pub fn world(&self) -> World<'_> {
        World {
            clipmap: &self.clipmap,
            ground: &self.ground,
            folds: &self.folds,
            shards: &self.shards,
            objects: &self.objects,
            pool: &self.pool,
        }
    }

    /// The ground's top at leaf column `g`, from whichever clip level holds it. What is placed
    /// is not read.
    pub fn top_at(&self, g: [i32; 2]) -> i32 {
        let (_, column, _, _) = clip::cell(
            &self.world(),
            Lookup::NONE,
            LEAF_CELL_ZOOM,
            [wrap_idx(LEAF_CELL_ZOOM, g[0]), g[1]],
        );
        clip::column_top(column)
    }
}

/// Folds the entries of the eye's clip levels in `clipmap`, one level at a time over its entry
/// table (`clip::fold`). Some entries are then left as a clip level that moved along one axis
/// would leave them, not yet folded again.
pub fn fold(
    clipmap: &mut [[i32; 4]],
    ground: &[u32],
    folds: &[u32],
    shards: &[[u32; 4]],
    pool: &[u32],
) {
    for l in 1..=JOINED_MIP_CNT {
        for cell_zoom in FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM {
            let entries: Vec<_> = {
                let w = World {
                    clipmap,
                    ground,
                    folds,
                    shards,
                    objects: &[0; 16],
                    pool,
                };
                (0..(ACROSS >> l) * (ACROSS >> l))
                    .map(|b| clip::fold(&w, cell_zoom, l, b))
                    .collect()
            };
            for (b, e) in entries.into_iter().enumerate() {
                let at = JOINED_AT
                    + (cell_zoom - FIRST_CELL_ZOOM) * JOINED_ENTRY_CNT
                    + clip::joined_idx(l, [0, 0]);
                clipmap[at as usize + b] = e.map(|v| v as i32);
            }
        }
    }
    for e in 0..(JOINED_ENTRY_CNT * EYE_CLIP_LEVEL_CNT) as usize {
        let h = mix(e as u32, [7, 11]);
        if h.is_multiple_of(5) {
            clipmap[JOINED_AT as usize + e][(h >> 8 & 1) as usize] ^= 1;
        }
    }
}

/// The service area's index, which holds each shard's nine (`area::nine`) at its key, as
/// `served` decides.
pub fn shards() -> Vec<[u32; 4]> {
    let mut shards = vec![[0u32; 4]; (area::SHARD_CELL_CNT / 4) as usize];
    for y in 0..1 << SHARD_ZOOM {
        for x in 0..1 << SHARD_ZOOM {
            let key = area::shard_key([x, y]) as usize;
            shards[key / 4][key % 4] = area::nine(served, [x, y]);
        }
    }
    shards
}

/// The world about an eye at `eye`, always from the same seed.
pub fn build(eye: [i32; 2]) -> Built {
    build_with_decors(eye, 0)
}

/// `build`, and `placed_decors` decors placed about the eye.
pub fn build_with_decors(eye: [i32; 2], placed_decors: u32) -> Built {
    build_with(eye, placed_decors, &[])
}

/// `build_with_decors`, with `leaves` placed last, each given as its zoom, node and word.
pub fn build_with(eye: [i32; 2], placed_decors: u32, leaves: &[(Zoom, [i32; 3], u32)]) -> Built {
    let shards = shards();

    // The clip levels and their bricks, each brick filled as a frame's fill fills it. Some
    // bricks are wanted and not yet held.
    let rows = (JOINED_AT + EYE_CLIP_LEVEL_CNT * JOINED_ENTRY_CNT) as usize;
    let mut clipmap = vec![[0i32; 4]; rows];
    let mut ground_words: Vec<u32> = Vec::new();
    let mut folds: Vec<u32> = Vec::new();
    let mut pool = vec![0u32; 1 << 23];
    let mut next_word = 1u32;
    let (mut lane, mut scratch) = (
        vec![0u32; BRICK_WORDS as usize],
        vec![0u32; SPANS_WORDS as usize],
    );
    let mut way_points = Vec::new();
    let (mut present_bricks, mut absent_bricks, mut way_bricks) = (0u32, 0u32, 0u32);
    for cell_zoom in FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM {
        let first = level_first(eye, cell_zoom);
        let laid = !UNLAID.contains(&cell_zoom) as i32;
        clipmap[(ORIGIN_AT + cell_zoom - FIRST_CELL_ZOOM) as usize] = [first[0], first[1], laid, 0];
        for k in 0..ENTRY_CNT {
            let q = [
                wrap_idx(
                    brick::bricks_zoom(cell_zoom),
                    first[0] + (k % ACROSS) as i32,
                ),
                first[1] + (k / ACROSS) as i32,
            ];
            let row = ((cell_zoom - FIRST_CELL_ZOOM) * ENTRY_CNT + clip::entry(q)) as usize;
            clipmap[row] = [q[0], q[1], 0, 0];
            let h = mix(cell_zoom, q);
            if h.is_multiple_of(7) {
                absent_bricks += 1;
                continue;
            }
            let lined = !(h >> 8).is_multiple_of(3);
            let brick = source(eye, cell_zoom, q, lined, &mut way_points);
            lane.fill(0);
            folds.extend_from_slice(&brick.four);
            let site = fill::Site {
                cell_zoom,
                place: [(q[0] & 7) as u32, (q[1] & 7) as u32],
                first: [
                    wrap_idx(cell_zoom, q[0] << BRICK_SHIFT),
                    q[1] << BRICK_SHIFT,
                ],
                shard: Shard::INSIDE,
                ele: &brick.ele,
                lines: &brick.lines,
            };
            fill::brick(&mut lane, &mut scratch, |i| brick.four[i as usize], &site);
            // A brick with spans keeps them in a run of the pool, as a fill keeps them.
            let words = fill::spans_run_words(&scratch);
            if words != 0 {
                let run = next_word as usize;
                fill::lay_spans(&scratch, &mut |k, v| pool[run + k as usize] = v);
                lane[ROOT_AT as usize] = next_word;
                next_word += words;
                way_bricks += 1;
            }
            let slot = present_bricks;
            ground_words.extend_from_slice(&lane);
            clipmap[row][2] = slot as i32 + 1;
            present_bricks += 1;
        }
    }
    assert!(ground_words.len() as u32 / BRICK_WORDS == present_bricks);
    // Some cells of each brick are laid void, with no ground of their own, both under ways and
    // away from them. Under a way, such a cell is what the way stands on; elsewhere it is
    // nothing.
    for slot in 0..present_bricks {
        for i in 0..BRICK_CELL_CNT {
            if mix(slot + 1000, [i as i32, 3]).is_multiple_of(29) {
                let w = (slot * BRICK_WORDS + OWN_AT + i / OWN_CELL_CNT) as usize;
                ground_words[w] &= !(0xff << (8 * (i % OWN_CELL_CNT)));
            }
        }
    }
    // Slot 0 gets a brick with spans, read by no node except through a place that names slot 0,
    // so a read of it where a place holds no brick would show.
    let words = BRICK_WORDS as usize;
    let treed = (0..present_bricks as usize)
        .find(|&s| ground_words[s * words + ROOT_AT as usize] != 0)
        .expect("a brick with spans");
    if treed != 0 {
        let (zero, rest) = ground_words.split_at_mut(treed * words);
        zero[..words].swap_with_slice(&mut rest[..words]);
        let cells = BRICK_CELL_CNT as usize;
        let (zero, rest) = folds.split_at_mut(treed * cells);
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
        "{present_bricks} bricks held, {absent_bricks} absent, {way_bricks} with ways over {next_word} words"
    );
    let objects_none = vec![0u32; 16];
    fold(&mut clipmap, &ground_words, &folds, &shards, &pool);

    // The ground's top at leaf column `g`, from whichever clip level holds it.
    let top_at = |g: [i32; 2]| {
        let w = World {
            clipmap: &clipmap,
            ground: &ground_words,
            folds: &folds,
            shards: &shards,
            objects: &objects_none,
            pool: &pool,
        };
        let (_, column, _, _) = clip::cell(
            &w,
            Lookup::NONE,
            LEAF_CELL_ZOOM,
            [wrap_idx(LEAF_CELL_ZOOM, g[0]), g[1]],
        );
        clip::column_top(column)
    };

    // What is placed about the eye and further off, each kind as a leaf of its own.
    let mut draw = Draw(0x1234_5679);
    let mut placing = Placing::new();
    let mut placed: Vec<(Zoom, [i32; 3])> = Vec::new();
    let mut folded: Vec<[i32; 3]> = Vec::new();
    for i in 0..2400u32 {
        let reach = if i % 4 == 0 { 3000 } else { 240 };
        let g = [
            eye[0] + draw.within(-reach, reach),
            (eye[1] + draw.within(-reach, reach)).max((40 << 16) + 2),
        ];
        let t = top_at(g);
        let leaf = |zoom: Zoom, dz: i32| {
            let c = |v: i32| {
                if zoom >= LEAF_CELL_ZOOM {
                    v << (zoom - LEAF_CELL_ZOOM)
                } else {
                    v >> (LEAF_CELL_ZOOM - zoom)
                }
            };
            (zoom, [wrap_idx(zoom, c(g[0])), c(g[1]), c(t + dz)])
        };
        let mut lay = |(zoom, c): (Zoom, [i32; 3]), word: u32| {
            placing.put(zoom, c, word);
            placed.push((zoom, c));
        };
        match i % 12 {
            // Finer than a cell: the eight nodes of one cell, each a byte's value. Case 8
            // fills all eight, case 9 the lower half, and case 10 lays air over bytes. Case 11
            // lays decor's value alone, which fills none.
            8..=11 => {
                let (z, c) = leaf(25, 0);
                let at = [c[0] & !1, c[1] & !1, c[2] & !1];
                for k in 0..8u32 {
                    let word = match i % 12 {
                        8 => object::leaf_word(5 + k * 3 + i % 7, 0),
                        9 if k < 4 => object::leaf_word(6 + k * 2, 0),
                        10 if k < 4 => object::leaf_word(40 + k, 0),
                        10 if k % 2 == 0 => object::leaf_word(Sentinel::Air as u32, 0),
                        11 => object::leaf_word(0x1500 + k, i),
                        _ => continue,
                    };
                    let d = [(k & 1) as i32, (k >> 1 & 1) as i32, (k >> 2) as i32];
                    lay((z, [at[0] + d[0], at[1] + d[1], at[2] + d[2]]), word);
                }
                folded.push([g[0], g[1], c[2] >> 1]);
            }
            // Case 0 places an object's color and case 1 a ground's palette entry, each with
            // its handle over its value.
            0 => lay(
                leaf(LEAF_CELL_ZOOM, draw.within(0, 4)),
                object::leaf_word(
                    match i / 12 % 3 {
                        0 => value::OBJECT[1],
                        1 => value::OBJECT[0],
                        _ => 0x101 + i % 40,
                    },
                    i,
                ),
            ),
            1 => lay(
                leaf(LEAF_CELL_ZOOM, 0),
                object::leaf_word(
                    match i / 12 % 3 {
                        0 => value::GROUND[1],
                        1 => value::GROUND[0],
                        _ => 5 + i % 30,
                    },
                    i,
                ),
            ),
            // Air, where a cube is cut out of the ground.
            2 => lay(
                leaf(LEAF_CELL_ZOOM, -draw.within(1, 5)),
                object::leaf_word(Sentinel::Air as u32, 0),
            ),
            // A leaf over several cells.
            3 => lay(
                leaf(22, draw.within(0, 9)),
                object::leaf_word(0x200 + i % 40, 0),
            ),
            // Finer than a cell: some of its cube.
            4 => {
                let (z, c) = leaf(FINEST - (i / 12 % 2), 0);
                for k in 0..1 + draw.next() % 40 {
                    let d = |b: u32| ((k >> b & 3) as i32) & 3;
                    let word = object::leaf_word(0x120 + i % 5, 0);
                    lay((z, [c[0] + d(0), c[1] + d(2), c[2] + d(4)]), word);
                }
            }
            // A fine leaf of a byte's value, with air beside it.
            5 => {
                let (z, c) = leaf(25, 1);
                lay((z, c), object::leaf_word(20 + i % 9, 0));
                lay(
                    (z, [c[0] + 1, c[1], c[2]]),
                    object::leaf_word(Sentinel::Air as u32, 0),
                );
                lay((z, [c[0], c[1] + 1, c[2]]), object::leaf_word(0x140, 0));
            }
            // A value that is no voxel, as decor's is.
            6 => lay(leaf(LEAF_CELL_ZOOM, 2), object::leaf_word(0x1234, i)),
            // A large leaf further off.
            _ => lay(
                leaf(18 + i % 5, draw.within(-40, 60)),
                object::leaf_word(0x180, 0),
            ),
        }
    }
    // Lone leaves of the finest zoom, each alone in its node of zoom 21. A fold there has a
    // kind and no share.
    let mut lone = Vec::new();
    for k in 0..300 {
        let g = [(-127 << 16) + 16 * k, (43 << 16) + 400 + 16 * (k % 40)];
        let p = [g[0], g[1], top_at(g) + 30];
        let at = [wrap_idx(FINEST, p[0] << 3), p[1] << 3, p[2] << 3];
        let val = if k % 2 == 0 {
            0x150
        } else {
            Sentinel::Air as u32
        };
        placing.put(FINEST, at, object::leaf_word(val, 0));
        lone.push(p);
    }
    // Leaves in the upper half of a node over a high bridge at the leaves, with the ground
    // under them. The box of what is placed and the bridge's box both fall short of the node
    // along Z.
    let mut over_bridges = Vec::new();
    for &(cell_zoom, line, p) in &way_points {
        let side = 16;
        if cell_zoom == LEAF_CELL_ZOOM && line == 3 && (3..8).contains(&p[2].rem_euclid(side)) {
            let z = p[2].div_euclid(side) * side + side / 2 + side / 4;
            for at in [[p[0], p[1], z], [p[0] + 3, p[1] - 3, z + 1]] {
                let c = [wrap_idx(LEAF_CELL_ZOOM, at[0]), at[1], at[2]];
                placing.put(LEAF_CELL_ZOOM, c, object::leaf_word(0x170, 0));
            }
            over_bridges.push(p);
        }
    }
    // Parts of cells placed where no clip level holds the ground under them.
    let mut groundless = Vec::new();
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
            object::leaf_word(if k % 2 == 0 { 9 } else { 0x130 }, 0),
        );
        groundless.push(leaf);
    }
    // Decor placed on the ground about the eye, each with its handle over its value, for
    // tests/decor.rs.
    let mut decors = Vec::new();
    let mut scatter = Draw(0x51ed_270b);
    for k in 0..placed_decors {
        let reach = if k % 3 == 0 { 2000 } else { 200 };
        let g = [
            eye[0] + scatter.within(-reach, reach),
            (eye[1] + scatter.within(-reach, reach)).max((40 << 16) + 2),
        ];
        let at = [
            wrap_idx(LEAF_CELL_ZOOM, g[0]),
            g[1],
            top_at(g) + scatter.within(0, 2),
        ];
        placing.put(
            LEAF_CELL_ZOOM,
            at,
            object::leaf_word(DECORS[k as usize % DECORS.len()].id, 7000 + k),
        );
        decors.push(at);
    }
    for &(zoom, c, word) in leaves {
        placing.put(zoom, c, word);
    }
    let objects = placing.words();
    Built {
        decors,
        clipmap,
        ground: ground_words,
        folds,
        shards,
        objects,
        pool,
        way_points,
        placed,
        folded,
        lone,
        over_bridges,
        groundless,
        draw,
    }
}

/// The world's one query asked of every node a walk visits, with nothing kept. A walk over it
/// answers what a walk must answer whatever the node cache holds.
pub struct Asking<'a> {
    pub w: World<'a>,
    pub cuts: [object::Cut; 2],
}

impl Query for Asking<'_> {
    type At = Inheritance;

    fn root(&mut self, node: [i32; 3]) -> Inheritance {
        Inheritance::of(&self.w, (0, node))
    }

    fn child(&mut self, at: &Inheritance, zoom: Zoom, node: [i32; 3]) -> Inheritance {
        at.child(&self.w, (zoom, node))
    }

    fn node(
        &mut self,
        n: &mut Inheritance,
        zoom: Zoom,
        c: [i32; 3],
        range: [Zoom; 2],
        _: f32,
    ) -> In<[u32; 2]> {
        let mut resident = self.w;
        world::node(
            &mut resident,
            &self.w,
            &self.w,
            self.w.pool,
            n,
            (zoom, c),
            range,
            self.cuts,
        )
    }
}

/// What the world answers of a node with nothing handed down to it, and what it hands on. This
/// is what a stackless walk asks.
pub fn truth(
    w: World,
    cuts: [object::Cut; 2],
    zoom: Zoom,
    c: [i32; 3],
    range: [Zoom; 2],
) -> (In<[u32; 2]>, [u32; 3]) {
    let mut resident = w;
    let mut at = Inheritance::of(&w, (zoom, c));
    let a = world::node(
        &mut resident,
        &w,
        &w,
        w.pool,
        &mut at,
        (zoom, c),
        range,
        cuts,
    );
    (a, at.words())
}

/// The Q3 position of `units`, with a fraction that lies off every edge of a step.
pub fn q8(units: [i32; 3]) -> [i32; 3] {
    [
        units[0].wrapping_shl(FRAC_W) | 5,
        units[1].wrapping_shl(FRAC_W) | 77,
        units[2].wrapping_shl(FRAC_W) | 13,
    ]
}

/// The world's buffers on the device.
pub struct WorldBuffer {
    pub clip: wgpu::Buffer,
    pub ground: wgpu::Buffer,
    pub folds: wgpu::Buffer,
    pub shards: wgpu::Buffer,
    pub placed: wgpu::Buffer,
    pub pool: wgpu::Buffer,
}

pub fn store_of() -> wgpu::BufferUsages {
    wgpu::BufferUsages::STORAGE
}
