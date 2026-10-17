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
use pxl_portable::fill::{self, column_of, level_of, put};
use pxl_portable::ground;
use pxl_portable::lattice::GROUND;
use pxl_portable::decor::{Decor, COUNTS};
use pxl_portable::placed::{self, Cut, Held, Lanes, BLOCK, PLACED, SPLIT};
use pxl_portable::scatter::{self, drawn, Grounds, GROWN_AT, GROWN_WORDS, PLACED_BLOCK, SQUARE_BLOCKS};
use pxl_portable::window::{self, Squares, FIRST_CELL, SLOTS, SQUARE, SQUARE_SHIFT, SQUARE_WORDS};
use pxl_portable::window::{Ran, FRESH_WORDS, RUNS_AT};
use pxl_portable::ways;
use pxl_portable::{at, mask, wrap_index, LEAF_CELL, SHARD_ZOOM, UNIT_ZOOM};
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

/// Square `square`'s first word in the ground's buffer (pxl-portable
/// `window::SQUARE_WORDS`).
fn square_at(square: u32) -> u32 {
    square * SQUARE_WORDS
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
    fn column(&self, cell: u32, slot: u32, i: u32) -> u32 {
        column_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), i)
    }
    fn level(&self, cell: u32, slot: u32, j: u32, at: [u32; 2]) -> i32 {
        level_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), j, at)
    }
}

/// The ways' runs `ways_cs` wrote in the eye's windows.
impl Ran for World<'_> {
    fn ran(&self, cell: u32, slot: u32, r: u32) -> u32 {
        at(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot) + RUNS_AT + r)
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

    /// The bound over the node of the window `cell` reads its cells from,
    /// the finest holding them present (`window::bounded`), as the CPU reads
    /// it (pxl-portable `Held::relief`).
    fn relief(&mut self, cell: u32, (zoom, c): (u32, [i32; 3])) -> ([i32; 2], bool) {
        let (r, _, here) = window::bounded(self, held_to(cell), (zoom, [c[0], c[1]]), FIRST_CELL);
        (r, here)
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
            // Carved: air as the caves answer it where the grid's ground fills,
            // which the air of a way beside it recolours; else only a way in
            // the block's own column writes in it.
            let carved = value == VOID && cell.block(zoom, c[2], false).0 != VOID;
            if carved || window::runs_in(self, &found, zoom, [c[0], c[1]]) {
                let ways = Ways { world: *self, zoom, own: [c[0], c[1]], cell, found };
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
/// the block's index's low bits, for `face_cs` to find the block by and
/// write its face bits over, then the nodes its descent asked about; where
/// it met nothing, the first two zero.
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
    let w3 = if steps { met.visits } else { folds };
    UVec4::new(value & 0xffff | kind << 16 | face << 18 | zoom << 21, s.t.to_bits(), low_bits(c), w3)
}

/// Bits of a block's index the lane keeps on each axis, enough to tell it
/// from the block its ray's hit point rounds to, which f32 holds to within
/// a few blocks.
const LOW_BITS: u32 = 10;

/// The greatest integer at most `v`.
fn floor(v: f32) -> i32 {
    let i = v as i32;
    if (i as f32) > v { i - 1 } else { i }
}

fn low_bits(c: [i32; 3]) -> u32 {
    let m = (1 << LOW_BITS) - 1;
    (c[0] & m) as u32 | ((c[1] & m) as u32) << LOW_BITS | ((c[2] & m) as u32) << (2 * LOW_BITS)
}

/// The block at `zoom` a ray stopped on, entering across `axis`, `t` along
/// it, its index's low bits `low` (`low_bits`): the block its hit point
/// rounds to, moved half a block into it across the face it entered,
/// corrected to the one whose low bits the cast kept.
fn hit_block(ray: &cast::Ray, t: f32, axis: u32, zoom: u32, low: u32) -> [i32; 3] {
    let side = cast::Place { zoom, corner: [0; 3] }.side();
    let half = (1 << LOW_BITS) / 2;
    let mut c = [0; 3];
    let mut a = 0;
    while a < 3 {
        let d = pxl_portable::axis(ray.direction, a as u32);
        let into = if a as u32 == axis { if d > 0.0 { 0.5 * side } else { -0.5 * side } } else { 0.0 };
        let rel = ray.frac[a] + d * t + into;
        let near = if zoom <= UNIT_ZOOM {
            (ray.unit[a] + floor(rel)) >> (UNIT_ZOOM - zoom)
        } else {
            (ray.unit[a] << (zoom - UNIT_ZOOM)) + floor(rel * (1 << (zoom - UNIT_ZOOM)) as f32)
        };
        let kept = ((low >> (LOW_BITS * a as u32)) & ((1 << LOW_BITS) - 1)) as i32;
        c[a] = near + ((kept - near + half) & ((1 << LOW_BITS) - 1)) - half;
        a += 1;
    }
    [wrap_index(zoom, c[0]), c[1], c[2]]
}

/// The face bits of each ray's block the cast found (`word`): the blocks
/// about it asked at its zoom, each once (pxl-portable `mask`), as a
/// chunk's box takes them, over the lane's index bits. A pass of its own, so
/// the cast's kernel holds the descent alone.
#[spirv(compute(threads(8, 8, 1)))]
pub fn face_cs(
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
    let i = (1 + id.y * across.x + id.x) as usize;
    if i >= lane.len() {
        return;
    }
    // SAFETY: just bounded by the lane's own length.
    let w = unsafe { *lane.get_unchecked(i) };
    if (w.x >> 16) & 3 == 0 {
        return;
    }
    let (ray, _) = eye_ray(eye, at(tangents, id.x), at(tangents, across.x + id.y));
    let mut world = World {
        window,
        ground,
        shards,
        overwrites,
        eye,
    };
    let (axis, zoom) = (((w.x >> 18) & 7) / 2, (w.x >> 21) & 63);
    let c = hit_block(&ray, f32::from_bits(w.y), axis, zoom, w.z);
    let filled = world.about(zoom, c);
    // SAFETY: as above.
    unsafe { *lane.get_unchecked_mut(i) = UVec4::new(w.x, w.y, mask::mask(filled), w.w) };
}

/// Where every ray from the eye starts alike (pxl-portable `cast::start`),
/// as `start_cs` writes it for `cast_cs`: its node's zoom and corner, what
/// was handed it, then the stack down to it, an `At` two words.
const START_WORDS: u32 = 4 + 2 * (1 + cast::DEPTH as u32);

fn at_words(a: At) -> [u32; 2] {
    [a.cave.word(), a.placed.word()]
}

fn at_of(w: &[u32], k: u32) -> At {
    At {
        cave: Cave::of_word(at(w, k)),
        placed: Held::of_word(at(w, k + 1)),
    }
}

/// The ray through the middle of the view, and the zooms each distance asks.
fn eye_ray(eye: &Eye, tx: f32, ty: f32) -> (cast::Ray, Detail) {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let direction = cast::through(v3(eye.forward_far), v3(eye.right_tau), v3(eye.up), tx, ty);
    let ray = cast::Ray::of([eye.origin.x, eye.origin.y, eye.origin.z], direction, eye.forward_far.w);
    let across = eye.across_range;
    (ray, Detail { tau: eye.right_tau.w, range: [across.z, across.w] })
}

/// The nodes over where every ray from the eye starts alike, asked once a
/// frame (pxl-portable `cast::start`) for every ray of `cast_cs` to start
/// from, one thread.
#[spirv(compute(threads(1, 1, 1)))]
pub fn start_cs(
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] starts: &mut [u32],
) {
    let (ray, detail) = eye_ray(eye, 0.0, 0.0);
    let mut world = World {
        window,
        ground,
        shards,
        overwrites,
        eye,
    };
    let s = cast::start(&mut world, &ray, detail);
    put(starts, 0, s.place.zoom);
    put(starts, 1, s.place.corner[0] as u32);
    put(starts, 2, s.place.corner[1] as u32);
    put(starts, 3, s.place.corner[2] as u32);
    let w = at_words(s.at);
    put(starts, 4, w[0]);
    put(starts, 5, w[1]);
    let mut k = 0;
    while k < cast::DEPTH {
        let w = at_words(s.stack[k]);
        put(starts, 6 + 2 * k as u32, w[0]);
        put(starts, 7 + 2 * k as u32, w[1]);
        k += 1;
    }
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] starts: &mut [u32],
) {
    let across = eye.across_range;
    if id.x >= across.x || id.y >= across.y {
        return;
    }
    let (ray, detail) = eye_ray(eye, at(tangents, id.x), at(tangents, across.x + id.y));
    let mut world = World {
        window,
        ground,
        shards,
        overwrites,
        eye,
    };
    // Started where every ray from the eye starts alike (`start_cs`).
    let read: &[u32] = starts;
    let mut from = cast::Start {
        place: cast::Place {
            zoom: at(read, 0),
            corner: [at(read, 1) as i32, at(read, 2) as i32, at(read, 3) as i32],
        },
        at: at_of(read, 4),
        stack: [at_of(read, 4); cast::DEPTH],
    };
    let mut k = 0;
    while k < cast::DEPTH {
        from.stack[k] = at_of(read, 6 + 2 * k as u32);
        k += 1;
    }
    let met = cast::first_from(&mut world, &ray, detail, Until::Fills, from);
    let steps = at(&*lane, 0).z != 0;
    let w = word(&mut world, &ray, met, steps);
    let i = (1 + id.y * across.x + id.x) as usize;
    if i < lane.len() {
        // SAFETY: just bounded by the lane's own length.
        unsafe { *lane.get_unchecked_mut(i) = w };
    }
}

/// The columns' ground tops of the squares `fresh` names, the squares
/// handed their tiles' samples this turn, one a `z`: the square, and where
/// it lies in its tile, in squares, X low; a thread a column
/// (`ground::height`) off the samples handed there.
#[spirv(compute(threads(8, 8, 1)))]
pub fn heights_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
) {
    if id.x >= SQUARE || id.y >= SQUARE {
        return;
    }
    let base = square_at(at(fresh, FRESH_WORDS * id.z));
    let place = at(fresh, FRESH_WORDS * id.z + 1);
    fill::heights(ground, base, [place & 0xffff, place >> 16], [id.x, id.y]);
}

/// Level `level.x` of the squares `fresh` names, one a `z`: level 0 each
/// column's own, its top and its runs (`fill::level`), a higher one over the
/// level under it, written before (`window::join`): a dispatch a level, from
/// 0 up.
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
    fill::level(ground, base, at(fresh, FRESH_WORDS * id.z + 2), j, [id.x, id.y]);
}

/// The ways of the squares `fresh` names, one a `z`, a thread a column
/// (`x`, cells row by row): each column's spans off the square's lines
/// (pxl-portable `ways::column`) read along z as runs (`ways::runs`), from
/// its column word's first (`window::COLUMN_WORDS`). Three dispatches,
/// `pass.x` 0 to 2: each column counts its runs into its word; one thread a
/// square sums them in order into each column's first; each column writes
/// its runs from its first. Over `heights_cs`'s tops: a way is told from
/// their faces.
#[spirv(compute(threads(64, 1, 1)))]
pub fn ways_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] pass: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
) {
    let base = square_at(at(fresh, FRESH_WORDS * id.z));
    let place = at(fresh, FRESH_WORDS * id.z + 1);
    // The window's cells' zoom, as handed with the square: a focus's or a
    // destination's is not its index's.
    let cell = at(fresh, FRESH_WORDS * id.z + 2);
    fill::ways(ground, base, [place & 0xffff, place >> 16], cell, pass.x, id.x);
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
