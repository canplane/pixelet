// gpu-query-shader/lib.rs

//! The cast's compute entry (`cast_cs`): each camera-grid cell's ray down
//! the pyramid by pxl-portable's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it; and the ground a held tile is drawn from,
//! its cells' heights off the samples it was handed (`heights_cs`) and their
//! bound's levels (`bound_cs`), which the CPU reads back; and the decor the
//! ground grows in the eye's windows (`decor_cs`). Each entry is a
//! module of its own.

#![cfg_attr(target_arch = "spirv", no_std)]

use pxl_portable::area::{self, Area};
use pxl_portable::cast::{self, Detail, Ends, In, Kind, Met, Query, Until};
use pxl_portable::caves::Cave;
use pxl_portable::cube::{object, sentinel};
use pxl_portable::ground;
use pxl_portable::lattice::GROUND;
use pxl_portable::decor::{Decor, COUNTS};
use pxl_portable::placed::{self, Cut, Held, Lanes, BLOCK, PLACED, SPLIT};
use pxl_portable::scatter::{self, drawn, Grounds, GROWN_AT, GROWN_WORDS, PLACED_BLOCK, SQUARE_BLOCKS};
use pxl_portable::window::{self, Squares, ELE_AT, FIRST_CELL, HEIGHTS_AT, LEVELS_AT, SLOTS, SQUARE, SQUARE_SHIFT, SQUARE_WORDS, WAYS_AT};
use pxl_portable::window::{Ran, FRESH_WORDS, LINES_AT, RUNS_AT, RUNS_RUN_AT, RUNS_SQUARE, SQUARE_CELLS, WAYS_BINS};
use pxl_portable::ways;
use pxl_portable::{at, mask, wrap_index, LEAF_CELL, SHARD_ZOOM};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

/// The eye a cast is from (pxl-kernel view/lens.rs `gpu_cast`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Eye {
    /// The Q3 position, and a pad.
    pub origin: IVec4,
    /// The unit forward, and the reach.
    pub forward_far: Vec4,
    /// The unit right, and τ.
    pub right_tau: Vec4,
    /// The unit up, and a pad.
    pub up: Vec4,
    /// The cells across and up, and the zooms a block is asked at.
    pub across_range: UVec4,
    /// A preview's cut to air and its left-out leaf: X, Y, Z and the zoom,
    /// -1 for none.
    pub carve: IVec4,
    pub hide: IVec4,
}

/// A preview's entry as pxl-portable `placed` takes it.
fn cut(e: IVec4) -> Cut {
    Cut {
        zoom: e.w as u32,
        at: [e.x, e.y, e.z],
    }
}

const VOID: u32 = sentinel::void as u32;
const PLACED_AIR: u32 = PLACED_BLOCK | object::air as u32;
const UNKNOWN: u32 = sentinel::unknown as u32;

/// A cell's or a window's zoom, held to the eye's windows: past the leaves,
/// the leaves'. None coarser than the first is asked: a ray's range starts at
/// the top tile's cells (pxl-kernel view/lens.rs `gpu_cast`, `asked`).
fn held_to(cell: u32) -> u32 {
    if cell > LEAF_CELL {
        LEAF_CELL
    } else {
        cell
    }
}

/// Word `i` of `s` set to `v`, as `at` reads: totally, none past it, so no
/// store aborts.
fn put(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { *s.get_unchecked_mut(i as usize) = v };
    }
}

/// Square `square`'s first word in the ground's buffer (pxl-portable
/// `window::SQUARE_WORDS`).
fn square_at(square: u32) -> u32 {
    square * SQUARE_WORDS
}

/// Cell `i`'s height off the square at `base`: two a word, low first, the
/// half's sign carried up.
fn height_of(ground: &[u32], base: u32, i: u32) -> i32 {
    ((at(ground, base + HEIGHTS_AT + i / 2) >> (16 * (i % 2))) << 16) as i32 >> 16
}

/// Entry `at` of level `j` off the square at `base`.
fn level_of(ground: &[u32], base: u32, j: u32, [x, y]: [u32; 2]) -> i32 {
    at(ground, base + LEVELS_AT + window::level_at(j) + y * (SQUARE >> j) + x) as i32
}

/// A square being drawn this turn, on its own: what `bound_cs` folds.
struct Fresh<'a> {
    ground: &'a [u32],
    base: u32,
}

impl Squares for Fresh<'_> {
    fn present(&self, _: u32, _: [i32; 2]) -> (u32, bool) {
        (0, true)
    }
    fn look(&self, _: u32, _: u32, i: u32) -> u32 {
        at(self.ground, self.base + i)
    }
    fn height(&self, _: u32, _: u32, i: u32) -> i32 {
        height_of(self.ground, self.base, i)
    }
    fn way(&self, _: u32, _: u32, k: u32) -> u32 {
        at(self.ground, self.base + WAYS_AT + k)
    }
    fn level(&self, _: u32, _: u32, j: u32, at: [u32; 2]) -> i32 {
        level_of(self.ground, self.base, j, at)
    }
}

/// A square being filled, as the ways' rule reads it (pxl-portable
/// `window::Square`): its lines, looks and heights in its lane.
struct Filling<'a> {
    ground: &'a [u32],
    base: u32,
    cell: u32,
    place: [u32; 2],
}

impl window::Square for Filling<'_> {
    fn cell(&self) -> u32 {
        self.cell
    }
    fn place(&self) -> [u32; 2] {
        self.place
    }
    fn line(&self, k: u32) -> u32 {
        at(self.ground, self.base + LINES_AT + k)
    }
    fn looks(&self, i: u32) -> u32 {
        at(self.ground, self.base + i)
    }
    /// Off its tile's samples as `heights_cs` draws a cell (`ground::height`),
    /// the ring about the square too.
    fn height(&self, c: [i32; 2]) -> i32 {
        let (samples, base) = (self.ground, self.base);
        let ele = move |i: usize| ((at(samples, base + ELE_AT + i as u32 / 2) >> (16 * (i as u32 % 2))) << 16) as i32 >> 16;
        let first = [(self.place[0] * SQUARE) as i32, (self.place[1] * SQUARE) as i32];
        let center = pxl_portable::lattice::CELL_CENTER;
        ground::bicubic(ele, (first[0] + c[0]) as f32 + center, (first[1] + c[1]) as f32 + center)
    }
}

/// Column `c` of the square at `base`, its spans off its lines read along z
/// (pxl-portable `ways::column`, `ways::runs`): its runs written from the
/// `n`-th as `room` holds them, and how many it holds.
fn fill_column(ground: &mut [u32], (base, cell, place): (u32, u32, [u32; 2]), c: [i32; 2], n: u32, room: u32) -> u32 {
    let spans = ways::column(&Filling { ground, base, cell, place }, c).0;
    ways::runs(&spans, cell, move |k, w| {
        if n + k < room {
            put(ground, base + RUNS_AT + RUNS_RUN_AT + n + k, w);
        }
    })
}

/// The world as the GPU holds it (pxl-kernel scene/cast.rs fills it): the
/// ground's held tiles, the service area, the caves, and what is placed with
/// a preview's cuts. Decor is not here (pxl-portable `cast::Query`); the
/// ways are, their bound and their runs in each square's lane.
#[derive(Clone, Copy)]
pub struct World<'a> {
    /// Per window of the eye's, from the top's, a word a slot: the square
    /// it holds, and whether it is present (pxl-portable `window`).
    window: &'a [IVec4],
    /// Per square of every window, its looks as handed, its cells' heights
    /// (`heights_cs`), their levels (`bound_cs`), its tile's samples and
    /// lines as handed, and its ways' runs (`ways_cs`)
    /// (`window::SQUARE_WORDS`); the eye's windows first.
    ground: &'a [u32],
    /// The served shards: zoom, x, y.
    shards: &'a [IVec4],
    /// The overwrites' tree (pxl-kernel layer/object/store.rs): its heap
    /// lane, then its data lane from half the buffer.
    overwrites: &'a [u32],
    /// The eye, whose preview cuts are read where something placed or cut
    /// is asked about, not held through the descent.
    eye: &'a Eye,
}

impl Lanes for World<'_> {
    fn heap(&self, slot: u32) -> u32 {
        at(self.overwrites, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        at(self.overwrites, self.overwrites.len() as u32 / 2 + slot)
    }
}

impl Area for World<'_> {
    fn ends(&self) -> bool {
        true
    }
    fn shards(&self) -> u32 {
        self.shards.len() as u32
    }
    fn shard(&self, i: u32) -> (u32, [i32; 2]) {
        let s = at(self.shards, i);
        (s.x as u32, [s.y, s.z])
    }
}

impl World<'_> {
    /// Which of the blocks about the block `c` at `zoom` fill, as pxl-portable
    /// `mask::filled` names them: a column at a time, each read once (its
    /// cell, or the Earth past the area), its three blocks decided on values;
    /// what is placed asked of each block, the caves only where one may open
    /// it (`Cell::opens`).
    fn about(&mut self, zoom: u32, c: [i32; 3]) -> u32 {
        let (carve, hide) = (cut(self.eye.carve), cut(self.eye.hide));
        let mut filled = 0;
        let mut k = 0;
        while k < 9 {
            let n = [wrap_index(zoom, c[0] + (k % 3) as i32 - 1), c[1] + (k / 3) as i32 - 1];
            let past = zoom >= SHARD_ZOOM && area::past(self, area::tile_at(zoom, n));
            let (cell, _, _) = ground::Held::cell(self, zoom, [n[0], n[1], c[2]]);
            let mut dz = 0;
            while dz < 3 {
                let (bit, b) = (1 << (dz * 9 + k), [n[0], n[1], c[2] + dz as i32 - 1]);
                if mask::READS & bit != 0 {
                    let held = placed::cursor(self, PLACED, zoom, b);
                    let over = if held.holds() || self.eye.carve.w >= 0 {
                        placed::over(self, held, carve, hide, zoom, b, true)
                    } else {
                        placed::Over { says: placed::UNDER, block: VOID }
                    };
                    let fills = if over.says == BLOCK {
                        self.fills(&[PLACED_BLOCK | over.block, 0])
                    } else if past {
                        area::earth(b[2]) != VOID
                    } else {
                        cell.block(zoom, b[2], false).0 != VOID && !(cell.opens(zoom, b[2]) && Cave::of(zoom, b).hollow())
                    };
                    filled |= (fills as u32) * bit;
                }
                dz += 1;
            }
            k += 1;
        }
        filled
    }

    /// The ground's cell `g` at `cell`, off the finest window holding it
    /// present (`window::find`): its four looks, its height and the zoom it
    /// is read at; `void` at sea level where none holds it.
    fn cell_at(&self, cell: u32, g: [i32; 2]) -> (u32, i32, window::Found) {
        let (f, i) = window::found_at(self, cell, g, FIRST_CELL);
        if !f.here {
            return (VOID, 0, f);
        }
        (self.look(f.from, f.slot, i), self.height(f.from, f.slot, i), f)
    }

    /// The ground's cell `b` of `zoom` as the grid reads it, before the
    /// area's edge is laid: the cell drawing it, past the leaves a leaf's;
    /// and where it is held.
    fn ground_at(&self, zoom: u32, b: [i32; 2]) -> (u32, f32, [u32; 4], window::Found) {
        let read = held_to(zoom);
        let (w, h, f) = self.cell_at(read, [b[0] >> (zoom - read), b[1] >> (zoom - read)]);
        let (look, h, fold) = ground::read_cell(w, h, f.from);
        (look, h, fold, f)
    }

    /// Whether the leaf cell `l` is gone: past the area, or nothing known
    /// of it, its look off the window drawing it.
    fn gone(&self, l: [i32; 2]) -> bool {
        let (w, _, f) = self.cell_at(LEAF_CELL, l);
        area::past(self, area::tile_at(LEAF_CELL, l)) || ground::own(w, f.from) == UNKNOWN
    }
}

/// The eye's windows, as the GPU holds them.
impl Squares for World<'_> {
    /// The slot square `q` is held in, while it holds it present.
    fn present(&self, cell: u32, q: [i32; 2]) -> (u32, bool) {
        let s = window::slot(q);
        let h = at(self.window, (cell - FIRST_CELL) * SLOTS + s);
        (s, h.z != 0 && h.x == q[0] && h.y == q[1])
    }
    fn look(&self, cell: u32, slot: u32, i: u32) -> u32 {
        at(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot) + i)
    }
    fn height(&self, cell: u32, slot: u32, i: u32) -> i32 {
        height_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), i)
    }
    fn way(&self, cell: u32, slot: u32, k: u32) -> u32 {
        at(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot) + WAYS_AT + k)
    }
    fn level(&self, cell: u32, slot: u32, j: u32, at: [u32; 2]) -> i32 {
        level_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), j, at)
    }
}

/// The ways' runs `ways_cs` wrote in the eye's windows.
impl Ran for World<'_> {
    fn ran(&self, cell: u32, slot: u32, k: u32) -> u32 {
        at(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot) + RUNS_AT + k)
    }
}

/// The ways about block `own` the descent asks at `zoom`, as the windows
/// hold them (`window::ran`), off the window its ground's cell was read at
/// (`found`) and the columns about it read off it: none in a column past
/// the area or walled, as the rule writes none there (pxl-portable
/// `ways::at`) — the own column's wall its cell as read (`cell`), each
/// other's its own.
struct Ways<'w> {
    world: World<'w>,
    zoom: u32,
    own: [i32; 2],
    cell: ground::Cell,
    found: window::Found,
}

impl ways::Airs for Ways<'_> {
    fn at(&self, c: [i32; 2], z: i32) -> u32 {
        let v = window::ran(&self.world, &self.found, self.zoom, c, z, FIRST_CELL);
        if v == ways::NOTHING {
            return v;
        }
        let walled = if c[0] == self.own[0] && c[1] == self.own[1] {
            self.cell.look == UNKNOWN
        } else {
            self.zoom >= SHARD_ZOOM && area::past(&self.world, area::tile_at(self.zoom, c)) || self.world.column(self.zoom, c).0.look == UNKNOWN
        };
        if walled { ways::NOTHING } else { v }
    }
}

/// The ground as the GPU holds it, for pxl-portable's one answer for a node.
impl ground::Held for World<'_> {
    type Place = window::Found;

    /// The join of the bounds over the node of each window the zooms
    /// `cells` read their cells from, the finest holding them present at
    /// each (`window::bounded`), by the CPU's one join (pxl-portable
    /// `ground::relief_over`); none (`UNBOUNDED`) for a node wider than a
    /// window, which only splits more.
    fn relief(&mut self, cells: [u32; 2], (zoom, c): (u32, [i32; 3])) -> ([i32; 2], bool) {
        ground::relief_over(cells, held_to, |z| {
            let (r, _, here) = window::bounded(self, held_to(z), (zoom, [c[0], c[1]]), FIRST_CELL);
            (r, here)
        })
    }

    /// The block's column off the tile drawing that zoom's cells, the area's
    /// edge laid on it (pxl-kernel query/node.rs `drawn`).
    fn cell(&mut self, zoom: u32, c: [i32; 3]) -> (ground::Cell, window::Found, bool) {
        let (cell, found) = self.column(zoom, [c[0], c[1]]);
        (cell, found, true)
    }
}

impl World<'_> {
    /// Column `b` at `zoom` off the tile drawing that zoom's cells, the
    /// area's edge laid on it, and the window it is read off.
    fn column(&self, zoom: u32, b: [i32; 2]) -> (ground::Cell, window::Found) {
        let (look, h, fold, found) = self.ground_at(zoom, b);
        // The leaf cells about it read by their offset, each its look alone.
        let gone_at = |dx: i32, dy: i32| {
            let s = zoom - LEAF_CELL;
            self.gone([wrap_index(LEAF_CELL, (b[0] >> s) + dx), (b[1] >> s) + dy])
        };
        let cell = ground::Cell {
            look: area::edge(look, zoom, b, look == UNKNOWN, gone_at),
            top: ground::blocks(zoom, h),
            fold,
        };
        (cell, found)
    }
}

/// What a node is handed down on the GPU: its caves, and what is placed
/// there.
#[derive(Clone, Copy)]
pub struct At {
    cave: Cave,
    placed: Held,
}

/// What a node is on the GPU: its value, and for the ground the four looks
/// a coarse block folds, a byte each, `dx + 2·dy` (`ground::Cell`), what
/// the screen paints a hit with.
impl Query for World<'_> {
    type At = At;
    type Block = [u32; 2];
    const AIR: [u32; 2] = [VOID, 0];

    fn root(&mut self, node: [i32; 3]) -> At {
        At {
            cave: Cave::of(0, node),
            placed: placed::cursor(self, PLACED, 0, node),
        }
    }

    fn child(&mut self, at: &At, zoom: u32, node: [i32; 3]) -> At {
        At {
            cave: at.cave.child(zoom, node),
            placed: placed::child(self, at.placed, zoom, node),
        }
    }

    /// What is in the node `c` at `zoom`, the layers top down: what is
    /// placed, then the ground's one answer for a node (pxl-portable
    /// `ground::node`); asked as a block at `near` or finer, or where the
    /// world holds nothing finer — past the ground's cells, with nothing
    /// placed or carved finer in it.
    fn node(&mut self, at: &mut At, zoom: u32, c: [i32; 3], asked: [u32; 2], _t0: f32) -> In<[u32; 2]> {
        let mut block = zoom >= asked[1] || zoom >= LEAF_CELL;
        // Asked of what is placed only where something is, or a preview cuts.
        if at.placed.holds() || self.eye.carve.w >= 0 {
            let carve = cut(self.eye.carve);
            block = zoom >= asked[1] || zoom >= LEAF_CELL && placed::whole(at.placed, carve, zoom, c);
            let over = placed::over(self, at.placed, carve, cut(self.eye.hide), zoom, c, block);
            if over.says == BLOCK {
                return In {
                    kind: Kind::Block,
                    block: [PLACED_BLOCK | over.block, 0],
                };
            }
            if over.says == SPLIT {
                return In { kind: Kind::Split, block: [VOID, 0] };
            }
        }
        let (i, cell, found) = ground::node(self, at.cave, (zoom, c), asked, block);
        let (mut value, mut f) = i.block;
        // What the ways write over the ground's block (pxl-portable
        // `ways::block`), as the CPU lays them (pxl-spec block/sample.rs
        // `ways_over`): air, or a way's look whole.
        if block && i.kind == Kind::Block {
            // Off the window the ground's cell was read at: one find a block.
            if window::asks(self, &found, zoom, c) {
                let ways = Ways { world: *self, zoom, own: [c[0], c[1]], cell, found };
                // Carved: air as the caves answer it where the grid's ground fills.
                let carved = value == VOID && cell.block(zoom, c[2], false).0 != VOID;
                let way = ways::block(&ways, c, carved);
                if way != ways::NOTHING {
                    value = if way == ways::AIR { VOID } else { way };
                    f = [value; 4];
                }
            }
        }
        In {
            kind: i.kind,
            block: [value, f[0] & 0xff | (f[1] & 0xff) << 8 | (f[2] & 0xff) << 16 | (f[3] & 0xff) << 24],
        }
    }

    fn fills(&self, block: &[u32; 2]) -> bool {
        block[0] != VOID && block[0] != PLACED_AIR
    }
}

/// The world as decor's growth asks it on the GPU (pxl-portable `scatter`):
/// the cast's, and the catalog's rows as handed.
struct Growing<'a> {
    world: World<'a>,
    rows: &'a [Decor],
}

impl Lanes for Growing<'_> {
    fn heap(&self, slot: u32) -> u32 {
        self.world.heap(slot)
    }
    fn data(&self, slot: u32) -> u32 {
        self.world.data(slot)
    }
}

/// A cell off the window drawing it, and a block asked by the cast's
/// descent, from a node started as the CPU's world starts one (pxl-kernel
/// query/node.rs `World::start`): its caves and what is placed found down
/// its line from the top.
impl Grounds for Growing<'_> {
    type At = At;

    fn rows(&self) -> &[Decor] {
        self.rows
    }

    fn face(&mut self, cell: u32, g: [i32; 2]) -> (u32, i32, bool) {
        let (c, _, _) = ground::Held::cell(&mut self.world, cell, [g[0], g[1], 0]);
        (c.look, c.top, c.grounded())
    }

    fn start(&mut self, zoom: u32, c: [i32; 3]) -> At {
        At {
            cave: Cave::of(zoom, c),
            placed: placed::cursor(&self.world, PLACED, zoom, c),
        }
    }

    fn block(&mut self, at: &At, from: u32, zoom: u32, c: [i32; 3]) -> u32 {
        let mut at = *at;
        let mut z = from;
        while z < zoom {
            z += 1;
            let s = zoom - z;
            at = self.world.child(&at, z, [c[0] >> s, c[1] >> s, c[2] >> s]);
        }
        self.world.node(&mut at, zoom, c, [zoom, zoom], 0.0).block[0]
    }
}

/// The lane's cell for a ray: the value it stopped on, its kind (ground or
/// placed) and the face it entered through, then how far as f32 bits, then
/// the nodes its descent asked about; where it met nothing, the first two
/// zero.
fn word(world: &mut World, ray: &cast::Ray, met: Met<At, [u32; 2]>, steps: bool) -> UVec4 {
    if met.ends != Ends::Stop {
        return UVec4::new(0, 0, 0, met.visits);
    }
    let s = met.stop;
    let down = pxl_portable::axis(ray.direction, s.axis) > 0.0;
    let face = 2 * s.axis + down as u32;
    let [value, folds] = s.block;
    let kind = if value & PLACED_BLOCK != 0 { 2 } else { 1 };
    let (zoom, c) = (s.place.zoom, s.place.node(ray));
    // The hit's face bits, the blocks about it asked at its zoom, each once
    // (pxl-portable `mask`), as a chunk's box takes them.
    let filled = world.about(zoom, c);
    let w3 = if steps { met.visits } else { folds };
    UVec4::new(value & 0xffff | kind << 16 | face << 18 | zoom << 21, s.t.to_bits(), mask::mask(filled), w3)
}

#[spirv(compute(threads(8, 8, 1)))]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] lane: &mut [UVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] tangents: &[f32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
) {
    let across = eye.across_range;
    if id.x >= across.x || id.y >= across.y {
        return;
    }
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let direction = cast::through(v3(eye.forward_far), v3(eye.right_tau), v3(eye.up), at(tangents, id.x), at(tangents, across.x + id.y));
    let ray = cast::Ray::of([eye.origin.x, eye.origin.y, eye.origin.z], direction, eye.forward_far.w);
    let detail = Detail {
        tau: eye.right_tau.w,
        range: [across.z, across.w],
    };
    let mut world = World {
        window,
        ground,
        shards,
        overwrites,
        eye,
    };
    let met = cast::first(&mut world, &ray, detail, Until::Fills);
    let steps = at(&*lane, 0).z != 0;
    let w = word(&mut world, &ray, met, steps);
    let i = (1 + id.y * across.x + id.x) as usize;
    if i < lane.len() {
        // SAFETY: just bounded by the lane's own length.
        unsafe { *lane.get_unchecked_mut(i) = w };
    }
}

/// The cells' heights of the squares `fresh` names, the squares handed
/// their tiles' samples this turn, one a `z`, each two words: the square,
/// and where it lies in its tile, in squares, X low: each word's two cells
/// (`ground::height`) off the samples handed there.
#[spirv(compute(threads(8, 8, 1)))]
pub fn heights_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
) {
    if id.x >= SQUARE / 2 || id.y >= SQUARE {
        return;
    }
    let base = square_at(at(fresh, FRESH_WORDS * id.z));
    let place = at(fresh, FRESH_WORDS * id.z + 1);
    let first = [(place & 0xffff) * SQUARE, (place >> 16) * SQUARE];
    let w = {
        let samples: &[u32] = ground;
        let ele = move |i: usize| ((at(samples, base + ELE_AT + i as u32 / 2) >> (16 * (i as u32 % 2))) << 16) as i32 >> 16;
        let (x, y) = (first[0] + 2 * id.x, first[1] + id.y);
        let (a, b) = (ground::height(ele, [x, y]), ground::height(ele, [x + 1, y]));
        (a as u32 & 0xffff) | (b as u32) << 16
    };
    ground[(base + HEIGHTS_AT + id.y * (SQUARE / 2) + id.x) as usize] = w;
}

/// Level `level.x` of the squares `fresh` names, one a `z`, over the level
/// under it, written before (`window::join`), level 1 over the cells
/// `heights_cs` drew: a dispatch a level, from 1 up.
#[spirv(compute(threads(8, 8, 1)))]
pub fn bound_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] level: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
) {
    let j = level.x;
    let side = SQUARE >> j;
    if id.x >= side || id.y >= side {
        return;
    }
    let base = square_at(at(fresh, FRESH_WORDS * id.z));
    let w = {
        let under: &[u32] = ground;
        window::join(&Fresh { ground: under, base }, 0, 0, j, [id.x, id.y])
    };
    ground[(base + LEVELS_AT + window::level_at(j) + id.y * side + id.x) as usize] = w as u32;
}

/// The ways of the squares `fresh` names, one a `z`, a thread a column
/// (`x`, cells row by row): each column's spans off the square's lines
/// (pxl-portable `ways::column`) read along z as runs (`ways::runs`) into
/// its runs lane, cells in order, each cell's first in the table before
/// them (`window::RUNS_AT`). Four dispatches, `pass.x` 0 to 3: each
/// column counts its runs into its table word; one thread a square sums
/// them in order into each cell's first; each column writes its runs from
/// its first; a thread a bin writes its ways' word off them
/// (`window::bin_word`), which the levels join after. Over `heights_cs`'s
/// cells: a way is told from their faces.
#[spirv(compute(threads(64, 1, 1)))]
pub fn ways_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] pass: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
) {
    let (square, i) = (at(fresh, FRESH_WORDS * id.z), id.x);
    let base = square_at(square);
    let place = at(fresh, FRESH_WORDS * id.z + 1);
    // The window's cells' zoom, as handed with the square: a focus's or a
    // destination's is not its index's.
    let cell = at(fresh, FRESH_WORDS * id.z + 2);
    let table = base + RUNS_AT;
    if pass.x == 3 {
        if i < WAYS_BINS {
            let word = {
                let lane: &[u32] = ground;
                let filling = Filling { ground: lane, base, cell, place: [place & 0xffff, place >> 16] };
                window::bin_word(&filling, move |k: u32| at(lane, table + k), i)
            };
            put(ground, base + WAYS_AT + i, word as u32);
        }
        return;
    }
    if pass.x == 1 {
        if i == 0 {
            let (mut n, mut k) = (0, 0);
            while k < SQUARE_CELLS {
                let count = at(ground, table + k);
                put(ground, table + k, n);
                (n, k) = (n + count, k + 1);
            }
            put(ground, table + SQUARE_CELLS, n);
        }
        return;
    }
    if i >= SQUARE_CELLS {
        return;
    }
    let (writes, c) = (pass.x == 2, [(i % SQUARE) as i32, (i / SQUARE) as i32]);
    let first = if writes { at(ground, table + i) } else { 0 };
    let square = (base, cell, [place & 0xffff, place >> 16]);
    let n = fill_column(ground, square, c, first, if writes { RUNS_SQUARE } else { 0 });
    if !writes {
        put(ground, table + i, n);
    }
}

/// Decor the ground grows in the eye's windows, one candidate a thread: `x`
/// its index in its block, `y` the block among those meeting its square
/// (pxl-portable `scatter::block_of`), `z` the window's zoom over the first
/// one's, its slot and the kind. Each one that stands, stands on its cell
/// and grows there (`scatter::stands`, `stand`, `grows`) is appended to
/// `out` after its count (`scatter::GROWN_AT`, `GROWN_WORDS`), where it is
/// drawn at that window's zoom, the one the rays ask at its distance
/// (`scatter::drawn_at`). A kind is grown at a zoom where bit
/// `zoom − FIRST_CELL` of its `shown` word is set; the kinds are the
/// catalog's rows as handed (`rows`).
#[spirv(compute(threads(8, 8, 1)))]
pub fn decor_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shown: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] rows: &[Decor],
) {
    let kinds = rows.len() as u32;
    let (kind, square) = (id.z % kinds, id.z / kinds);
    let (cell, slot) = (window::eye_cell(square / SLOTS), square % SLOTS);
    if id.x >= COUNTS as u32 || id.y >= SQUARE_BLOCKS || cell > LEAF_CELL || at(shown, kind) >> (cell - FIRST_CELL) & 1 == 0 {
        return;
    }
    let held = at(window, (cell - FIRST_CELL) * SLOTS + slot);
    if held.z == 0 {
        return;
    }
    // The square's leaf cells.
    let s = LEAF_CELL - cell;
    let first = [(held.x << SQUARE_SHIFT) << s, (held.y << SQUARE_SHIFT) << s];
    let region = [first, [first[0] + (SQUARE << s) as i32, first[1] + (SQUARE << s) as i32]];
    let mut world = Growing {
        world: World {
            window,
            ground,
            shards,
            overwrites,
            eye,
        },
        rows,
    };
    // Only where the rays ask this zoom (`scatter::drawn_at`): the square
    // as its heights' bound has it, then each decor where it stands.
    let detail = Detail {
        tau: eye.right_tau.w,
        range: [eye.across_range.z, eye.across_range.w],
    };
    let origin = [eye.origin.x, eye.origin.y, eye.origin.z];
    let heights = ground::bound_of(world.world.level(cell, slot, window::LEVELS, [0, 0]));
    if !scatter::meets_band(detail, origin, region, heights, cell) {
        return;
    }
    let decor = &rows[kind as usize];
    let zoom = GROUND.read_zoom(cell);
    let (c, here) = scatter::candidate_of(decor, zoom, region, id.y, id.x);
    if !here || !scatter::stands(decor, zoom, &c) {
        return;
    }
    let (g, here) = scatter::stand(&mut world, decor, cell, &c);
    if !here || !scatter::drawn_at(detail, origin, g.at, cell) || !scatter::grows(&mut world, decor, cell, &g) {
        return;
    }
    // SAFETY: an atomic add on the buffer's first word, its count.
    let n = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut out[0], 1) };
    let i = (GROWN_AT + GROWN_WORDS * n) as usize;
    if i + 3 < out.len() {
        out[i] = kind | g.variant << 8 | g.turn << 16;
        out[i + 1] = g.at[0] as u32;
        out[i + 2] = g.at[1] as u32;
        out[i + 3] = g.at[2] as u32;
    }
}

/// The records `out` holds, placed decor's and what `decor_cs` grew: the
/// count, held to the records' room.
fn records(grown: &[u32]) -> u32 {
    let room = (grown.len() as u32 - GROWN_AT) / GROWN_WORDS;
    let n = at(grown, 0);
    if n < room {
        n
    } else {
        room
    }
}

/// Each record sized for the screen (pxl-portable `scatter::drawn`): a
/// model where its group's extent spans `MODEL_PX` at its distance, a
/// sprite down to `SPRITE_PX`, else nothing, or nothing where it is the
/// decor left out; its region written into its first word and counted.
#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_count_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grown: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] regions: &mut [u32],
) {
    if id.x >= records(grown) {
        return;
    }
    let i = (GROWN_AT + GROWN_WORDS * id.x) as usize;
    let w = grown[i];
    let (kind, variant) = (w & 0xff, (w >> 8) & 0xff);
    let corner = [grown[i + 1] as i32, grown[i + 2] as i32, grown[i + 3] as i32];
    let group = scatter::first_group(rows, kind) + variant;
    let hide = drawn::HIDE as usize;
    let hidden = params[hide + 3] != 0 && corner[0] == params[hide] as i32 && corner[1] == params[hide + 1] as i32 && corner[2] == params[hide + 2] as i32;
    let d = scatter::distance([eye.origin.x, eye.origin.y, eye.origin.z], corner);
    let d = if d > 1e-3 { d } else { 1e-3 };
    let px = f32::from_bits(params[(drawn::EXTENTS + group) as usize]) / (d * f32::from_bits(params[drawn::UPP as usize]));
    let region = if hidden {
        drawn::NONE
    } else if px >= f32::from_bits(params[drawn::MODEL_PX as usize]) {
        2 * group
    } else if px >= f32::from_bits(params[drawn::SPRITE_PX as usize]) {
        2 * group + 1
    } else {
        drawn::NONE
    };
    grown[i] = w & ((1 << drawn::REGION_SHIFT) - 1) | region << drawn::REGION_SHIFT;
    if region != drawn::NONE {
        // SAFETY: an atomic add on the region's count, a word of the buffer.
        unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut regions[region as usize], 1) };
    }
}

/// Each region laid after the one before (`decor_count_cs`'s counts), its
/// first slot kept for `decor_place_cs` and added to its draw uniform's
/// index, and its indirect args: a model region's indexed by its group's
/// mesh (`meshes`, its index count, none while it loads), a sprite region's
/// a strip of four corners an instance.
#[spirv(compute(threads(1, 1, 1)))]
pub fn decor_args_cs(
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] meshes: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] args: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] uniforms: &mut [u32],
) {
    let (mut first, mut r) = (0, 0);
    while r < drawn::REGIONS {
        let n = regions[r as usize];
        regions[(drawn::REGIONS + r) as usize] = first;
        uniforms[(r * drawn::UNIFORM_WORDS + drawn::UNIFORM_INDEX) as usize] += first;
        let a = (drawn::ARGS_WORDS * r) as usize;
        if r % 2 == 0 {
            args[a] = at(meshes, r / 2);
            args[a + 1] = n;
            args[a + 2] = 0;
            args[a + 3] = 0;
            args[a + 4] = 0;
        } else {
            args[a] = 4;
            args[a + 1] = n;
            args[a + 2] = 0;
            args[a + 3] = 0;
            args[a + 4] = 0;
        }
        first += n;
        r += 1;
    }
}

/// Each record its region drew a slot of: its box's base middle about the
/// anchor, its heading and scale, in the lanes the decor and impostor
/// shaders read, and the slot its own in the index list.
#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_place_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grown: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] block: &mut [u32],
) {
    if id.x >= records(grown) {
        return;
    }
    let i = GROWN_AT + GROWN_WORDS * id.x;
    let w = at(grown, i);
    let region = w >> drawn::REGION_SHIFT;
    if region == drawn::NONE {
        return;
    }
    // SAFETY: an atomic add on the region's next slot, a word of the buffer.
    let slot = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut regions[(drawn::REGIONS + region) as usize], 1) };
    let (kind, variant, turn) = (w & 0xff, (w >> 8) & 0xff, (w >> 16) & 0xff);
    let [across, deep, _] = rows[kind as usize].cells(variant, turn);
    let room = block.len() as u32 / (drawn::LANES + 1);
    if slot >= room {
        return;
    }
    let anchor = drawn::ANCHOR as usize;
    let i = i as usize;
    // X about the anchor the short way round: leaf cells wrap at 2^24.
    let x = (((grown[i + 1] as i32).wrapping_sub(params[anchor] as i32)) << 8 >> 8) as f32 + across as f32 / 2.0;
    let y = (grown[i + 2] as i32).wrapping_sub(params[anchor + 1] as i32) as f32 + deep as f32 / 2.0;
    let z = (grown[i + 3] as i32).wrapping_sub(params[anchor + 2] as i32) as f32;
    let (room, slot) = (room as usize, slot as usize);
    block[slot] = x.to_bits();
    block[room + slot] = y.to_bits();
    block[2 * room + slot] = z.to_bits();
    block[3 * room + slot] = ((turn % 4) as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    block[4 * room + slot] = 1.0f32.to_bits();
    block[drawn::LANES as usize * room + slot] = slot as u32;
}
