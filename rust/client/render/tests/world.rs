//! The world's one query by hand (frame/shader/world.wgsl) answers word for word as
//! `world::node` does: real world buffers made here on the CPU, as the fill and the
//! store lay them, nodes asked along whole descents of the Rust, each step asked again
//! of the hand query on a device, and every word compared, what the node hands down
//! included. Skipped where no adapter answers.

use std::collections::BTreeMap;

use pxl_content::cube::ground as palette;
use pxl_content::{cave, Pixelet};
use pxl_engine::answer::{self, In, Kind};
use pxl_engine::area::{self, Area, Shard};
use pxl_engine::clipmap::{
    self as clip, Bricks, Lookup, Presence, ACROSS, BRICK_CELLS, BRICK_SHIFT, BRICK_WORDS, ENTRIES,
    EYE_CLIP_LEVELS, FIRST_CELL, HANDED_WORDS, JOINED_AT, JOINED_ENTRIES, JOINED_LEVELS, LINES_AT,
    LINES_ENTRIES_AT, LINES_SEGMENTS_AT, ORIGIN_AT, PALETTE_AT, ROOT_AT, TREE_AT,
};
use pxl_engine::fill;
use pxl_engine::ground::{self, Resident};
use pxl_engine::lattice::TILE_CELLS;
use pxl_engine::overwrite::{self, Columns, Cut, BRANCH};
use pxl_engine::way::{self, tree, Segment, Stations};
use pxl_engine::world::{self, Inheritance};
use pxl_engine::{value, wrap_idx, Zoom, LEAF_CELL, SHARD_ZOOM};

/// The hash made-up content turns on.
fn mix(a: u32, c: [i32; 2]) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ 0x51ED_270B;
    for v in c {
        h = (h ^ v as u32).wrapping_mul(0x85EB_CA77).rotate_left(13);
    }
    h ^ h >> 15
}

/// A seeded stream.
struct Draw(u32);

impl Draw {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    /// In `[lo, hi)`.
    fn within(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u32) as i32
    }
}

/// The finest zoom asked: what is placed is held no finer (pxl-spec bake `OBJECT_FINEST`).
const FINEST: Zoom = LEAF_CELL + 3;

/// Where the eye is, a leaf cell: just east of the world's seam in X, so its finest clip
/// level starts east of it and its coarser ones west, a few cells over the service area's
/// south edge.
const EYE: [i32; 2] = [-(1 << 23) + 300, (40 << 16) + 30];

/// The shards served: west of the seam far past what the clip levels reach, a few east of
/// it, from row 40 to 44: an edge of the area within every clip level's reach but the west;
/// and a shard left out in it (`NOTCH`), whose corners only a diagonal leaf cell touches;
/// and the shard at the world's first node, no clip level's reach.
fn served(n: [i32; 2]) -> bool {
    (n[0] >= 100 || n[0] < -124) && (40..44).contains(&n[1]) && n != NOTCH || n == [0, 0]
}

/// The shard left out of the area.
const NOTCH: [i32; 2] = [-126, 41];

/// The leaf cells that touch the notch only at a corner, what each is named by a tally.
const CORNERS: [(&str, [i32; 2]); 4] = [
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
const SUNK_FROM: i32 = 42 << 16;

/// The clip levels wanted and not laid out.
const UNLAID: [Zoom; 2] = [15, 21];

/// The first brick of the clip level at `cell`.
fn level_first(cell: Zoom) -> [i32; 2] {
    let s = LEAF_CELL - cell;
    let first = clip::origin(cell, [EYE[0] >> s, EYE[1] >> s]);
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
fn spire(cell: Zoom, x: i32, y: i32) -> i32 {
    let s = BRICK_SHIFT + LEAF_CELL - cell;
    let mut most = 0;
    for finer in cell..=LEAF_CELL {
        let f = BRICK_SHIFT + LEAF_CELL - finer;
        let first = level_first(finer);
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
fn elevation(cell: Zoom, x: i32, y: i32) -> i32 {
    let (fx, fy) = (x as f32, y as f32);
    let hills = 300.0 * libm::sinf(fx * 1.3e-5) + 120.0 * libm::cosf(fy * 3.1e-5);
    let near = 40.0 * libm::sinf(fx * 0.002) + 30.0 * libm::cosf(fy * 0.0031);
    let fine = 8.0 * libm::sinf((fx + fy) * 0.017);
    let sunk = if y >= SUNK_FROM { -1500 } else { 0 };
    (hills + near + fine) as i32 + sunk + spire(cell, x, y)
}

/// A leaf cell's column at `cell` as `g` (`g` the cell's middle).
fn leaf_of(cell: Zoom, g: i32) -> i32 {
    let s = LEAF_CELL - cell;
    (g << s) + ((1 << s) >> 1)
}

/// The world as the GPU holds it, read as pxl-shader's `World` reads it.
#[derive(Clone, Copy)]
struct World<'a> {
    clipmap: &'a [[i32; 4]],
    ground: &'a [u32],
    palette: &'a [u32],
    shards: &'a [[u32; 4]],
    overwrites: &'a [u32],
    pool: &'a [u32],
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
type Point = (Zoom, u32, [i32; 3]);

/// A brick's record as a frame hands it: its four palette indices a cell, its tile's
/// samples, its lines. Its ways' middles are added to `ways`.
fn handed(fresh: &mut [u32], cell: Zoom, q: [i32; 2], lined: bool, ways: &mut Vec<Point>) {
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
            let e = elevation(cell, at(sx, first[0]) as i32, at(sy, first[1]) as i32);
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
            (elevation(cell, leaf(p[0], first[0]), leaf(p[1], first[1])) + lift) as i16
        };
        let width = 16 + (h >> 24) % 48;
        let kind = value::TRANSPORTS[0] + (h >> 4) % 18;
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
struct Placing {
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

/// Words of an ask and of what is told (world_asked_cs below).
const ASKED_WORDS: usize = 20;
const TOLD_WORDS: usize = 8;
const OP_NODE: u32 = 0;
const OP_ROOT: u32 = 1;
const OP_CHILD: u32 = 2;
const OP_HOLLOW: u32 = 3;

const ENTRY: &str = r"
@group(0) @binding(12) var<storage, read> asked: array<u32>;
@group(0) @binding(13) var<storage, read_write> told: array<u32>;

@compute @workgroup_size(64)
fn world_asked_cs(@builtin(global_invocation_id) id: vec3<u32>) {
  let a = id.x * 20u;
  if (a >= arrayLength(&asked)) {
    return;
  }
  let op = asked[a];
  let zoom = asked[a + 1u];
  let c = vec3<i32>(i32(asked[a + 2u]), i32(asked[a + 3u]), i32(asked[a + 4u]));
  let at = vec4<u32>(asked[a + 7u], asked[a + 8u], asked[a + 9u], asked[a + 10u]);
  var r = Answer(0u, vec2<u32>(0u), vec4<u32>(0u));
  switch op {
    case 0u: {
      let carve = vec4<i32>(i32(asked[a + 11u]), i32(asked[a + 12u]), i32(asked[a + 13u]), i32(asked[a + 14u]));
      let hide = vec4<i32>(i32(asked[a + 15u]), i32(asked[a + 16u]), i32(asked[a + 17u]), i32(asked[a + 18u]));
      r = world_node(at, zoom, c, asked[a + 5u], asked[a + 6u], carve, hide);
    }
    case 1u: {
      r.at = world_root(zoom, c);
    }
    case 2u: {
      r.at = world_child(at, zoom, c);
    }
    default: {
      r.asset.x = hollow(zoom, c);
    }
  }
  let t = id.x * 8u;
  told[t] = r.kind;
  told[t + 1u] = r.asset.x;
  told[t + 2u] = r.asset.y;
  told[t + 3u] = r.at.x;
  told[t + 4u] = r.at.y;
  told[t + 5u] = r.at.z;
  told[t + 6u] = r.at.w;
}
";

/// The asks made so far and what the Rust told of each.
#[derive(Default)]
struct Asks {
    asked: Vec<u32>,
    told: Vec<u32>,
}

impl Asks {
    #[expect(clippy::too_many_arguments, reason = "an ask's words, each its own")]
    fn push(
        &mut self,
        (op, zoom, c): (u32, Zoom, [i32; 3]),
        zooms: [Zoom; 2],
        at: [u32; 4],
        cuts: [Cut; 2],
        kind: u32,
        asset: [u32; 2],
        after: [u32; 4],
    ) {
        let word = |cut: Cut| {
            [
                cut.at[0] as u32,
                cut.at[1] as u32,
                cut.at[2] as u32,
                cut.zoom,
            ]
        };
        self.asked
            .extend([op, zoom, c[0] as u32, c[1] as u32, c[2] as u32]);
        self.asked.extend(zooms);
        self.asked.extend(at);
        self.asked.extend(word(cuts[0]));
        self.asked.extend(word(cuts[1]));
        self.asked.push(0);
        self.told.extend([kind, asset[0], asset[1]]);
        self.told.extend(after);
        self.told.push(0);
    }
}

/// How the answers fell, a kind of answer each.
#[derive(Default)]
struct Tally(BTreeMap<&'static str, u32>);

impl Tally {
    fn add(&mut self, name: &'static str, yes: bool) {
        *self.0.entry(name).or_default() += yes as u32;
    }
}

/// One descent to the node at `point` (`FINEST` cells), a step a zoom from `start`, nothing
/// handed it, to `stop`, the Rust's answer at each and what it hands on. `moving`, each node
/// has its own zooms, as a ray's interval gives them (`cast::walk`): mostly narrowing, its
/// near falling and its far rising, some jumping finer.
#[expect(clippy::too_many_arguments, reason = "a descent's own, each its own")]
fn descend(
    w: World,
    point: [i32; 3],
    [start, stop]: [Zoom; 2],
    [mut far, mut near]: [Zoom; 2],
    moving: bool,
    cuts: [Cut; 2],
    asks: &mut Asks,
    tally: &mut Tally,
    draw: &mut Draw,
    seen: &mut Lookup,
) {
    let idx = |z: Zoom| {
        let a = overwrite::ancestor_at(FINEST, point, FINEST - z);
        [wrap_idx(z, a[0]), a[1], a[2]]
    };
    let pool = tree::Read(w.pool);
    let root = idx(start);
    let mut at = Inheritance::of(&w, (start, root));
    asks.push(
        (OP_ROOT, start, root),
        [0, 0],
        [0; 4],
        [Cut::NONE; 2],
        0,
        [0, 0],
        at.words(),
    );
    tally.add("root", true);
    for z in start..=stop {
        let c = idx(z);
        if z > start {
            let before = at.words();
            at = at.child(&w, &pool, (z, c));
            asks.push(
                (OP_CHILD, z, c),
                [0, 0],
                before,
                [Cut::NONE; 2],
                0,
                [0, 0],
                at.words(),
            );
            tally.add("child", true);
            tally.add("child, ways' cursor held", at.ways.holds());
            if moving {
                match draw.next() % 8 {
                    0..=2 => {
                        near = near.saturating_sub(1 + draw.next() % 2).max(FIRST_CELL);
                        far = (far + 1).min(near);
                    }
                    3 | 4 => near = (near + 2 + draw.next() % 3).min(FINEST),
                    _ => {}
                }
                far = far.min(near);
            }
        }
        let zooms = [far, near];
        // Some handed where another descent's cells were held, as a pyramid's kept word may
        // name a brick since gone.
        if moving && *seen != Lookup::NONE && draw.next().is_multiple_of(16) {
            at.ground = *seen;
            tally.add("handed another descent's place", true);
        }
        let before = at;
        let mut held = w;
        let got = world::node(&mut held, &w, &w, &pool, &mut at, (z, c), zooms, cuts);
        asks.push(
            (OP_NODE, z, c),
            zooms,
            before.words(),
            cuts,
            got.kind as u32,
            got.asset,
            at.words(),
        );
        *seen = at.ground;
        tell(w, before, (z, c), zooms, cuts, got, at, tally);
    }
}

/// Which kinds of answer one fell among.
#[expect(clippy::too_many_arguments, reason = "an answer and what it was asked")]
fn tell(
    w: World,
    before: Inheritance,
    (z, c): (Zoom, [i32; 3]),
    zooms: [Zoom; 2],
    cuts: [Cut; 2],
    got: In<[u32; 2]>,
    at: Inheritance,
    t: &mut Tally,
) {
    let shard = at.shard;
    let v = answer::val(got.asset[0]);
    let is_cell = got.kind == Kind::Cell;
    // What was asked of the clip levels against what a node over it found.
    let asked = clip::held_to(if z >= zooms[1] { z } else { zooms[1] });
    let above = before.ground;
    if above != Lookup::NONE && asked <= z + clip::LEVELS {
        t.add("asked coarser than above's cell", asked < above.cell());
        t.add("asked coarser than above's clip", asked < above.from());
        t.add("asked finer by 2 or more", asked >= above.cell() + 2);
    }
    if asked > z + clip::LEVELS && !UNLAID.contains(&asked) {
        let up = asked - z;
        let g = [
            wrap_idx(asked, c[0] << up) >> BRICK_SHIFT,
            (c[1] << up) >> BRICK_SHIFT,
        ];
        let (first, side) = (level_first(asked), 1 << (up - clip::LEVELS));
        let d = [
            wrap_idx(clip::bricks_zoom(asked), g[0] - first[0]),
            g[1] - first[1],
        ];
        t.add("a block ending at its level's first, x", d[0] + side == 0);
        t.add("a block ending at its level's first, y", d[1] + side == 0);
        t.add("a block starting on its level's first, x", d[0] == 0);
        t.add("a block starting on its level's first, y", d[1] == 0);
    }
    t.add(
        "asked of a level not laid",
        asked > z + clip::LEVELS && UNLAID.contains(&asked),
    );
    if (pxl_engine::TOP_ZOOM..SHARD_ZOOM).contains(&z) && got.kind == Kind::Split {
        let mut place = before.ground;
        let coarsest = ground::coarsest(z, zooms[0]).min(zooms[1]);
        let cells = [coarsest, zooms[1]];
        let (b, _, here) = clip::bound_over(&w, &mut place, cells, (z, [c[0], c[1]]));
        t.add(
            "wider than a shard, all under the sea",
            here && b[0][1] < pxl_engine::SEA_LEVEL,
        );
    }
    if z >= LEAF_CELL && is_cell && !shard.past() {
        let leaf = [c[0] >> (z - LEAF_CELL), c[1] >> (z - LEAF_CELL)];
        for (name, corner) in CORNERS {
            let on = wrap_idx(LEAF_CELL, corner[0]) == leaf[0] && corner[1] == leaf[1];
            t.add(name, on && area::is_wall(shard, z, [c[0], c[1]]));
        }
    }
    t.add(
        "past the leaves, the node 0, 0, 0, coarser than asked",
        z >= LEAF_CELL && c == [0, 0, 0] && z < zooms[1],
    );
    t.add("past the poles or the world's height", {
        let row = 1 << (z.max(1) - 1);
        let side = if z > SHARD_ZOOM {
            1 << (z - SHARD_ZOOM - 1)
        } else {
            1
        };
        !(-row..row).contains(&c[1]) || !(-side..side).contains(&c[2])
    });
    if got.kind != Kind::Cell && z >= pxl_engine::TOP_ZOOM && !is_cell {
        let (mut held, mut place) = (w, before.ground);
        let (i, _, bound) = ground::node(&mut held, shard, &mut place, (z, c), zooms, false);
        let ways = way::slab(bound, [ground::coarsest(z, zooms[0]), zooms[1]]);
        t.add(
            "ways' span rounded to none",
            bound[0] < bound[1] && ways[0] == ways[1],
        );
        t.add(
            "ways' span rounded to none, strictly in the node",
            bound[0] < bound[1] && ways[0] == ways[1] && ground::meets(ways, (z, c[2]), zooms[1]),
        );
        let z_first = got.asset[1] >> 10 & 1023;
        t.add(
            "split, no ground, ways' and placed's firsts over 0",
            got.kind == Kind::Split
                && i.kind == Kind::Cell
                && at.object.is_folded()
                && ways[0] < ways[1]
                && z_first > 0
                && z_first < answer::BOX_STEPS / 2,
        );
    }
    if is_cell && at.object.is_folded() {
        let word = w.data(at.object.slot());
        t.add(
            "a fold of none, of a kind",
            overwrite::high(word) == 0 && overwrite::low(word) != 0,
        );
    }
    t.add("split", got.kind == Kind::Split);
    t.add(
        "split, a box short of whole",
        got.kind == Kind::Split && got.asset != answer::BOX_WHOLE,
    );
    t.add("lack", got.kind == Kind::Lacks);
    t.add("cell", is_cell);
    t.add(
        "cell, air",
        is_cell && v == value::VOID && got.asset[0] == 0,
    );
    if is_cell && answer::is_placed(got.asset[0]) && z >= FIRST_CELL && !shard.past() {
        let (mut held, mut place) = (w, before.ground);
        let (i, _, _) = ground::node(&mut held, shard, &mut place, (z, c), zooms, true);
        t.add("cell over a lack, placed", i.kind == Kind::Lacks);
    }
    t.add("cell, placed", is_cell && answer::is_placed(got.asset[0]));
    t.add(
        "cell, placed air",
        is_cell && answer::is_placed(got.asset[0]) && v == value::AIR,
    );
    t.add("cell, an object's", is_cell && value::objects(v));
    let placed = is_cell && answer::is_placed(got.asset[0]);
    t.add(
        "cell, placed, the first object's",
        placed && v == value::OBJECTS[0],
    );
    t.add(
        "cell, placed, the first ground's",
        placed && v == value::GROUNDS[0],
    );
    t.add(
        "cell, kept short of its cube",
        is_cell && answer::fill_eighths(got.asset[0]) < 8 && v != 0,
    );
    t.add("cell, a way's", is_cell && value::transports(v));
    t.add("cell, stone", is_cell && v == palette::stone as u32);
    t.add("cell, soil", is_cell && v == palette::soil as u32);
    t.add("cell, sandstone", is_cell && v == palette::sandstone as u32);
    t.add("cell, unknown", is_cell && v == value::UNKNOWN);
    t.add(
        "cell, coarser than asked, past the leaves",
        is_cell && z >= LEAF_CELL && z < zooms[1] && v != 0,
    );
    t.add(
        "cell, four differ",
        is_cell && {
            let [a, b, c, d] = ground::palette_indices(got.asset[1]);
            a != b || b != c || c != d
        },
    );
    t.add("past the area", shard.past());
    if z >= SHARD_ZOOM && is_cell {
        let wall = !shard.past() && area::is_wall(shard, z, [c[0], c[1]]);
        t.add("cell, on the wall", wall);
        t.add("cell, the wall's", wall && v == value::UNKNOWN);
    }
    t.add("ways' cursor held", at.ways.holds());
    // What the caves took of it, off the ground's own answer.
    if is_cell && z >= FIRST_CELL && !shard.past() {
        let mut held = w;
        let mut place = before.ground;
        let (i, _, _) = ground::node(&mut held, shard, &mut place, (z, c), zooms, true);
        if i.kind == Kind::Cell {
            t.add(
                "cell, some of it cave",
                i.asset[1] > 0 && i.asset[1] < answer::FULL,
            );
            t.add("cell, all of it cave", i.asset[1] >= answer::FULL);
        }
    }
    // Whether a cut changed it.
    if cuts != [Cut::NONE; 2] {
        let (mut held, mut plain, pool) = (w, before, tree::Read(w.pool));
        let without = world::node(
            &mut held,
            &w,
            &w,
            &pool,
            &mut plain,
            (z, c),
            zooms,
            [Cut::NONE; 2],
        );
        t.add("a cut changed it", without != got);
    }
}

fn read(device: &wgpu::Device, queue: &wgpu::Queue, from: &wgpu::Buffer) -> Vec<u32> {
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: from.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut e = device.create_command_encoder(&Default::default());
    e.copy_buffer_to_buffer(from, 0, &staging, 0, from.size());
    queue.submit([e.finish()]);
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let v = staging.slice(..).get_mapped_range().unwrap();
    v.as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

/// The module of `source`; its errors said where it is refused.
fn taken(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let info = pollster::block_on(module.get_compilation_info());
    let refused = pollster::block_on(scope.pop()).is_some();
    let lines: Vec<&str> = source.lines().collect();
    for m in &info.messages {
        let line = m.location.map_or(0, |l| l.line_number as usize);
        eprintln!(
            "{label}: {:?} at {line}: {}\n  {}",
            m.message_type,
            m.message,
            lines.get(line.wrapping_sub(1)).copied().unwrap_or("")
        );
    }
    assert!(!refused, "{label} refused");
    module
}

#[test]
fn hand_world_answers_as_the_rust() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no adapter: skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();

    // The service area's index, a cell's nine by its key.
    let mut shards = vec![[0u32; 4]; (area::SHARD_CELLS / 4) as usize];
    for y in 0..1 << SHARD_ZOOM {
        for x in 0..1 << SHARD_ZOOM {
            let key = area::shard_key([x, y]) as usize;
            shards[key / 4][key % 4] = area::nine(served, [x, y]);
        }
    }

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
        let first = level_first(cell);
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
            handed(&mut fresh, cell, q, lined, &mut way_points);
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
    // Their entries, folded a level at a time.
    let overwrites_none = vec![0u32; 16];
    for l in 1..=JOINED_LEVELS {
        for cell in FIRST_CELL..=LEAF_CELL {
            let entries: Vec<_> = {
                let w = World {
                    clipmap: &clipmap,
                    ground: &ground_words,
                    palette: &palette_words,
                    shards: &shards,
                    overwrites: &overwrites_none,
                    pool: &pool,
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

    // Some entries as a clip level moved along one axis left them, not yet folded again.
    for e in 0..(JOINED_ENTRIES * EYE_CLIP_LEVELS) as usize {
        let h = mix(e as u32, [7, 11]);
        if h.is_multiple_of(5) {
            clipmap[JOINED_AT as usize + e][(h >> 8 & 1) as usize] ^= 1;
        }
    }

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
            EYE[0] + draw.within(-reach, reach),
            (EYE[1] + draw.within(-reach, reach)).max((40 << 16) + 2),
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
    let w = World {
        clipmap: &clipmap,
        ground: &ground_words,
        palette: &palette_words,
        shards: &shards,
        overwrites: &overwrites,
        pool: &pool,
    };

    // The points descents are sent to, in leaf cells, each a share of the kinds a
    // descent must meet, some at the zooms that meet them.
    let mut points: Vec<[i32; 3]> = Vec::new();
    let mut asked_at: Vec<([i32; 3], [Zoom; 2])> = Vec::new();
    // About the surface, at every scale the clip levels hold.
    for i in 0..3000 {
        let reach = 128 << (i % 12);
        let g = [
            EYE[0] + draw.within(-reach, reach),
            EYE[1] + draw.within(-reach, reach),
        ];
        points.push([g[0], g[1], top_at(g) + draw.within(-4, 4)]);
    }
    // In the caves: leaves they take, found by asking under the ground about the eye.
    let mut caves = 0;
    for _ in 0..200_000 {
        let g = [
            EYE[0] + draw.within(-200, 200),
            EYE[1] + draw.within(0, 300),
        ];
        let z = top_at(g) - draw.within(4, 60);
        let c = [wrap_idx(LEAF_CELL, g[0]), g[1], z];
        if cave::hollow((LEAF_CELL, c)) >= answer::FULL {
            points.push([g[0], g[1], z]);
            caves += 1;
            if caves == 800 {
                break;
            }
        }
    }
    // On the ways: about each line's middle, at its cells' zoom, a clip level's alone.
    for &(cell, _, p) in &way_points {
        let s = 1 << (LEAF_CELL - cell);
        for _ in 0..12 {
            let d = [
                draw.within(-3, 3) * s,
                draw.within(-3, 3) * s,
                draw.within(-3, 2) * s,
            ];
            asked_at.push(([p[0] + d[0], p[1] + d[1], p[2] + d[2]], [cell, cell]));
        }
    }
    for &p in &over_bridges {
        for d in -2..3 {
            asked_at.push(([p[0], p[1], p[2] + d], [LEAF_CELL - 1, LEAF_CELL]));
        }
    }
    for &p in &lone {
        asked_at.push((p, [20, 21]));
        asked_at.push((p, [18, 18 + draw.next() % 4]));
    }
    // At each corner of the notch.
    for (_, corner) in CORNERS {
        for _ in 0..150 {
            let leaves = LEAF_CELL + draw.next() % 4;
            let z = top_at(corner) + draw.within(-6, 3);
            asked_at.push(([corner[0], corner[1], z], [leaves - 1, leaves]));
        }
    }
    // Past the poles, some on rows a served one's key's low bits name, and past the height.
    for _ in 0..1500 {
        let x = (-127 << 16) + draw.within(-20_000, 20_000);
        let row = 40 + draw.within(0, 4);
        let wide = if draw.next().is_multiple_of(2) {
            256
        } else {
            -256
        };
        let inside = (row << 16) + draw.within(0, 1 << 16);
        points.push([x, inside + (wide << 16), draw.within(-400, 400)]);
        let sign = if draw.next().is_multiple_of(2) { 1 } else { -1 };
        let z = sign * ((1 << 15) + draw.within(1, 8000));
        points.push([x, inside, z]);
    }
    // Past the pole on the row that names row 0, beside the shard served there.
    for _ in 0..300 {
        let g = [
            draw.within(-1 << 16, 2 << 16),
            (256 << 16) + draw.within(0, 1 << 16),
        ];
        points.push([g[0], g[1], draw.within(-200, 200)]);
    }
    // Under the sea, in nodes wider than a shard.
    for _ in 0..800 {
        let g = [
            [-128, -126, 126][draw.next() as usize % 3] << 16,
            [42, 43][draw.next() as usize % 2] << 16,
        ];
        let g = [
            g[0] + draw.within(0, 1 << 17),
            g[1] + draw.within(0, 1 << 16),
        ];
        let near = FIRST_CELL + draw.next() % 3;
        asked_at.push(([g[0], g[1], draw.within(-200, 200)], [near - 1, near]));
    }
    // The first node of the world along x and y, its finer zooms' nodes.
    for z in -2..3 {
        for near in [LEAF_CELL + 1, LEAF_CELL + 2, FINEST] {
            for _ in 0..12 {
                asked_at.push(([0, 0, z], [near - 1, near]));
            }
        }
    }
    // On a cell what is placed folds in, asked as that cell.
    for &p in &unheld {
        asked_at.push((p, [LEAF_CELL - 1, LEAF_CELL]));
    }
    for &p in &folds {
        asked_at.push((p, [LEAF_CELL - 1, LEAF_CELL]));
    }
    // About the ways and what is placed, far over and under them.
    for _ in 0..1500 {
        let g = [
            EYE[0] + draw.within(-400, 400),
            EYE[1] + draw.within(0, 400),
        ];
        points.push([g[0], g[1], top_at(g) + draw.within(-2000, 2000)]);
    }
    // At the edges of every clip level, a node wider than a brick its blocks reach or not.
    for cell in FIRST_CELL..=LEAF_CELL {
        let s = LEAF_CELL - cell;
        let row = clipmap[(ORIGIN_AT + cell - FIRST_CELL) as usize];
        let (lo, side) = ([row[0], row[1]], ACROSS as i32);
        for _ in 0..500 {
            let along = |d: &mut Draw, v: i32| ((v + d.within(-2, side + 2)) << BRICK_SHIFT) << s;
            let edge = |d: &mut Draw, v: i32| {
                let k = d.within(0, 2) * side + d.within(-2, 2);
                ((v + k) << BRICK_SHIFT) << s
            };
            for g in [
                [along(&mut draw, lo[0]), edge(&mut draw, lo[1])],
                [edge(&mut draw, lo[0]), along(&mut draw, lo[1])],
            ] {
                let far = cell.saturating_sub(draw.next() % 3);
                let g = [wrap_idx(LEAF_CELL, g[0]), g[1]];
                let z = top_at(g) + 40 + draw.within(0, 1000);
                asked_at.push(([g[0], g[1], z], [far, cell]));
            }
        }
    }
    // Under the ground about the eye: its strata at the leaves, the caves' share of coarser
    // cells.
    for _ in 0..3000 {
        let g = [
            EYE[0] + draw.within(-250, 250),
            EYE[1] + draw.within(0, 250),
        ];
        let leaves = LEAF_CELL + draw.next() % 4;
        asked_at.push((
            [g[0], g[1], top_at(g) - draw.within(1, 70)],
            [leaves - 1, leaves],
        ));
        let g = [
            EYE[0] + draw.within(-4000, 4000),
            EYE[1] + draw.within(0, 4000),
        ];
        let near = 17 + draw.next() % 7;
        asked_at.push((
            [g[0], g[1], top_at(g) - draw.within(16, 300)],
            [near - 2, near],
        ));
    }
    // On what is placed.
    for &(zoom, c) in placed.iter().step_by(2) {
        let leaf = |v: i32| {
            if zoom >= LEAF_CELL {
                v >> (zoom - LEAF_CELL)
            } else {
                v << (LEAF_CELL - zoom)
            }
        };
        points.push([leaf(c[0]), leaf(c[1]), leaf(c[2]) + draw.within(-1, 2)]);
    }
    // On the wall, along the area's south edge, and past it.
    for _ in 0..4000 {
        let g = [
            EYE[0] + draw.within(-250, 250),
            (40 << 16) + draw.within(-1, 2),
        ];
        let leaves = LEAF_CELL + draw.next() % 4;
        asked_at.push((
            [g[0], g[1], top_at(g) + draw.within(-6, 3)],
            [leaves - 1, leaves],
        ));
    }
    // On the wall along its east and north edges, off a coarser clip level's bricks.
    for _ in 0..2000 {
        for g in [
            [
                (-124 << 16) + draw.within(-2, 2),
                EYE[1] + draw.within(-8000, 8000),
            ],
            [
                EYE[0] + draw.within(-8000, 8000),
                (44 << 16) + draw.within(-2, 2),
            ],
        ] {
            let leaves = LEAF_CELL + draw.next() % 4;
            asked_at.push((
                [g[0], g[1], top_at(g) + draw.within(-6, 3)],
                [leaves - 1, leaves],
            ));
        }
    }
    // Past the area on every side, and inside it where no clip level holds it.
    for _ in 0..1000 {
        let row = if draw.next().is_multiple_of(2) {
            draw.within(-1 << 23, 40 << 16)
        } else {
            draw.within(-1 << 17, 1 << 17)
        };
        points.push([draw.next() as i32 >> 4, row, draw.within(-400, 400)]);
    }
    for _ in 0..800 {
        let g = [
            (110 << 16) + draw.within(-1 << 16, 1 << 16),
            (42 << 16) + draw.within(-1 << 16, 1 << 16),
        ];
        points.push([g[0], g[1], draw.within(-300, 300)]);
    }

    // Each down one line, its zooms by how far off it lies.
    let mut asks = Asks::default();
    let mut tally = Tally::default();
    let mut seen = Lookup::NONE;
    let by_its_distance = points.iter().map(|p| {
        let off = (p[0] - EYE[0]).abs().max((p[1] - EYE[1]).abs()).max(1);
        let band = (32 - off.leading_zeros()).saturating_sub(8);
        (
            *p,
            (LEAF_CELL - band.min(LEAF_CELL - FIRST_CELL)).max(FIRST_CELL),
        )
    });
    let all: Vec<_> = by_its_distance
        .map(|(p, near)| (p, None, near))
        .chain(
            asked_at
                .iter()
                .map(|&(p, [far, near])| (p, Some(far), near)),
        )
        .collect();
    eprintln!("{} points", all.len());
    for (i, &(p, far, near)) in all.iter().enumerate() {
        let targeted = far.is_some();
        let near = if far.is_some() {
            near
        } else {
            match draw.next() % 4 {
                0 => FIRST_CELL + draw.next() % (FINEST - FIRST_CELL + 1),
                1 => LEAF_CELL + draw.next() % 4,
                _ => near,
            }
        };
        let far = far.unwrap_or(match draw.next() % 5 {
            0 => near,
            1 => near.saturating_sub(1),
            2 => near.saturating_sub(3),
            3 => near.saturating_sub(7),
            _ => 4,
        });
        let stop = (near + draw.next() % 2).min(FINEST);
        let point = [
            (p[0] << 3) + draw.within(0, 8),
            (p[1] << 3) + draw.within(0, 8),
            (p[2] << 3) + draw.within(0, 8),
        ];
        let ancestor = |z: Zoom| {
            let a = overwrite::ancestor_at(FINEST, point, FINEST - z);
            [wrap_idx(z, a[0]), a[1], a[2]]
        };
        let carve = match i % 6 {
            1 | 2 => {
                let z = stop.saturating_sub(draw.next() % 3).max(1);
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            3 => {
                let z = (stop + 1 + draw.next() % 2).min(FINEST);
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            _ => Cut::NONE,
        };
        let hide = match i % 6 {
            2 => {
                let (zoom, at) = placed[draw.next() as usize % placed.len()];
                Cut { zoom, at }
            }
            4 => {
                // The leaf over it, where one lies at a zoom a leaf is put at.
                let z = [22, 24, 25, 26, FINEST][draw.next() as usize % 5];
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            5 => {
                let z = stop;
                Cut {
                    zoom: z,
                    at: ancestor(z),
                }
            }
            _ => Cut::NONE,
        };
        // Those sent to a kind of answer at its zooms keep them, half of them.
        let moving = !targeted || i % 2 == 0;
        // Most from the world's root; some from a node with nothing known over it.
        let start = if i % 3 == 0 {
            draw.next() % (stop + 1)
        } else {
            0
        };
        descend(
            w,
            point,
            [start, stop],
            [far, near],
            moving,
            [carve, hide],
            &mut asks,
            &mut tally,
            &mut draw,
            &mut seen,
        );
    }
    // The caves' one share, of nodes at every zoom.
    let mut hollows = 0u32;
    for k in 0..6000u32 {
        let z = 14 + k % 14;
        let g = [
            EYE[0] + draw.within(-600, 600),
            EYE[1] + draw.within(-600, 600),
        ];
        let up = 24i32 - z as i32;
        let v = |x: i32| if up >= 0 { x >> up } else { x << -up };
        let c = [wrap_idx(z, v(g[0])), v(g[1]), v(draw.within(-300, 300))];
        let share = cave::hollow((z, c));
        hollows += (share > 0) as u32;
        asks.push(
            (OP_HOLLOW, z, c),
            [0, 0],
            [0; 4],
            [Cut::NONE; 2],
            0,
            [share, 0],
            [0; 4],
        );
    }
    tally.0.insert("a share of cave, asked apart", hollows);
    // Every block of pipe cells a piece holds some of, its nearest blocks told apart.
    let mut partial = 0u32;
    for _ in 0..200_000 {
        if partial == 500 {
            break;
        }
        let c = [
            draw.within(-1 << 20, 1 << 20),
            draw.within(-1 << 20, 1 << 20),
            draw.within(-1 << 8, 1 << 8),
        ];
        let share = cave::hollow((cave::PIPE, c));
        if share == 0 || share >= answer::FULL {
            continue;
        }
        partial += 1;
        for b in 0..64 {
            let d = [b & 3, b >> 2 & 3, b >> 4];
            let at = [4 * c[0] + d[0], 4 * c[1] + d[1], 4 * c[2] + d[2]];
            let z = cave::PIPE + 2;
            asks.push(
                (OP_HOLLOW, z, at),
                [0, 0],
                [0; 4],
                [Cut::NONE; 2],
                0,
                [cave::hollow((z, at)), 0],
                [0; 4],
            );
        }
    }
    tally
        .0
        .insert("pipe cells of a partial piece, every block", partial);

    let asked_n = asks.asked.len() / ASKED_WORDS;
    eprintln!("{asked_n} asks");
    for (name, n) in &tally.0 {
        eprintln!("  {n:>8}  {name}");
    }

    // Asked again of the hand query, a chunk at a time.
    use wgpu::util::DeviceExt as _;
    let buffer = |words: &[u32], usage: wgpu::BufferUsages| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            // SAFETY: every u32's bytes are u8s, in the slice's own lifetime.
            contents: unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), words.len() * 4) },
            usage: usage | wgpu::BufferUsages::COPY_SRC,
        })
    };
    let flat = |rows: &[[u32; 4]]| rows.iter().flatten().copied().collect::<Vec<u32>>();
    let clip_flat: Vec<u32> = clipmap.iter().flatten().map(|&v| v as u32).collect();
    let store = wgpu::BufferUsages::STORAGE;
    let (clip_buf, ground_buf, palette_buf, shard_buf, placed_buf, pool_buf) = (
        buffer(&clip_flat, store),
        buffer(&ground_words, store),
        buffer(&palette_words, store),
        buffer(&flat(&shards), store),
        buffer(&overwrites, store),
        buffer(&pool, store),
    );
    let words = [
        include_str!(concat!(env!("OUT_DIR"), "/shader/walk_word.wgsl")),
        include_str!(concat!(env!("OUT_DIR"), "/shader/world_word.wgsl")),
    ]
    .join("\n");
    let words = words.as_str();
    let hand = include_str!(concat!(env!("OUT_DIR"), "/shader/world.wgsl"));
    let source = [words, hand, ENTRY].join("\n");
    let module = taken(&device, "world", &source);
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("world"),
        layout: None,
        module: &module,
        entry_point: Some("world_asked_cs"),
        compilation_options: Default::default(),
        cache: None,
    });
    // It lies beside the walk's, as an entry will read both.
    taken(
        &device,
        "world and walk",
        &[
            words,
            include_str!(concat!(env!("OUT_DIR"), "/shader/constant.wgsl")),
            include_str!(concat!(env!("OUT_DIR"), "/shader/walk.wgsl")),
            hand,
        ]
        .join("\n"),
    );

    const CHUNK: usize = 1 << 14;
    let (mut same, mut differ, mut first) = (0u64, 0u64, None);
    let mut differ_by_op = [0u32; 4];
    for (n, (a, t)) in asks
        .asked
        .chunks(CHUNK * ASKED_WORDS)
        .zip(asks.told.chunks(CHUNK * TOLD_WORDS))
        .enumerate()
    {
        let asked_buf = buffer(a, store);
        let told_buf = buffer(&vec![0u32; t.len()], store);
        let entries = [
            (1, &clip_buf),
            (2, &ground_buf),
            (4, &palette_buf),
            (5, &shard_buf),
            (6, &placed_buf),
            (7, &pool_buf),
            (12, &asked_buf),
            (13, &told_buf),
        ]
        .map(|(b, buf): (u32, &wgpu::Buffer)| wgpu::BindGroupEntry {
            binding: b,
            resource: buf.as_entire_binding(),
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut e = device.create_command_encoder(&Default::default());
        {
            let mut pass = e.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(((a.len() / ASKED_WORDS) as u32).div_ceil(64), 1, 1);
        }
        queue.submit([e.finish()]);
        let got = read(&device, &queue, &told_buf);
        for (k, (want, have)) in t.chunks(TOLD_WORDS).zip(got.chunks(TOLD_WORDS)).enumerate() {
            if want == have {
                same += 1;
            } else {
                differ += 1;
                let ask = &a[k * ASKED_WORDS..(k + 1) * ASKED_WORDS];
                differ_by_op[ask[0] as usize] += 1;
                first.get_or_insert((n * CHUNK + k, ask.to_vec(), want.to_vec(), have.to_vec()));
            }
        }
    }
    eprintln!("{same} answered alike, {differ} differ, by op (node, root, child, hollow): {differ_by_op:?}");
    if let Some((k, ask, want, have)) = &first {
        eprintln!("first differing, ask {k}:\n  asked {ask:?}\n  rust  {want:?}\n  hand  {have:?}");
    }
    assert_eq!(differ, 0, "the hand query differs");
    assert_eq!(same as usize, asked_n);

    // Every kind of answer met, at least half of what this seeded run meets.
    let least = [
        ("a block ending at its level's first, x", 462),
        ("a block ending at its level's first, y", 3040),
        ("a block starting on its level's first, x", 3298),
        ("a block starting on its level's first, y", 16412),
        ("a cut changed it", 17392),
        ("a fold of none, of a kind", 514),
        ("a share of cave, asked apart", 1172),
        ("asked coarser than above's cell", 14194),
        ("asked coarser than above's clip", 8071),
        ("asked finer by 2 or more", 12882),
        ("asked of a level not laid", 41692),
        ("cell", 149937),
        ("cell over a lack, placed", 403),
        ("cell, a way's", 905),
        ("cell, air", 98194),
        ("cell, all of it cave", 773),
        ("cell, an object's", 7380),
        ("cell, coarser than asked, past the leaves", 5599),
        ("cell, four differ", 5351),
        ("cell, kept short of its cube", 3708),
        ("cell, on the wall", 3463),
        ("cell, placed", 22888),
        ("cell, placed air", 15209),
        ("cell, placed, the first ground's", 9),
        ("cell, placed, the first object's", 3),
        ("cell, sandstone", 56),
        ("cell, soil", 351),
        ("cell, some of it cave", 1602),
        ("cell, stone", 2457),
        ("cell, the wall's", 906),
        ("cell, unknown", 10461),
        ("child", 629146),
        ("child, ways' cursor held", 23870),
        ("handed another descent's place", 7843),
        ("lack", 1226),
        ("past the area", 139883),
        ("past the leaves, the node 0, 0, 0, coarser than asked", 11),
        ("past the poles or the world's height", 22339),
        ("pipe cells of a partial piece, every block", 250),
        ("root", 36391),
        ("split", 514374),
        ("split, a box short of whole", 170538),
        ("split, no ground, ways' and placed's firsts over 0", 4),
        ("wall by a diagonal, north-east", 191),
        ("wall by a diagonal, north-west", 191),
        ("wall by a diagonal, south-east", 189),
        ("wall by a diagonal, south-west", 194),
        ("ways' cursor held", 34123),
        ("ways' span rounded to none", 437),
        ("ways' span rounded to none, strictly in the node", 182),
        ("wider than a shard, all under the sea", 113),
    ];
    for (name, n) in least {
        let got = tally.0.get(name).copied().unwrap_or(0);
        assert!(got >= n, "too few of {name} to say: {got}, wanted {n}");
    }
}
