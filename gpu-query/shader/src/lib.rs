// gpu-query-shader/lib.rs

//! The cast's compute entry (`cast_cs`): each camera-grid cell's ray down
//! the pyramid by pxl-portable's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it; and the ground a held tile is drawn from,
//! its cells' heights off the samples it was handed (`heights_cs`) and their
//! bound's levels (`bound_cs`), which the CPU reads back. Each entry is a
//! module of its own. The harness's entries are behind its feature
//! (`harness`).

#![cfg_attr(target_arch = "spirv", no_std)]

#[cfg(feature = "harness")]
pub mod harness;

use pxl_portable::area::{self, Area};
use pxl_portable::cast::{self, Detail, Ends, In, Kind, Met, Query, Until};
use pxl_portable::caves::Cave;
use pxl_portable::cube::{object, sentinel, Ground};
use pxl_portable::ground;
use pxl_portable::lattice::GROUND;
use pxl_portable::placed::{self, Cut, Held, Lanes, BLOCK, PLACED, SPLIT};
use pxl_portable::window::{self, Squares, ELE_AT, HEIGHTS_AT, LEVELS_AT, SLOTS, SQUARE, SQUARE_WORDS, WAYS_AT};
use pxl_portable::{at, mask, wrap_index, LEAF_CELL, SHARD_ZOOM};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
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
/// A block's word when what is placed answered it, over its value.
const PLACED_BLOCK: u32 = 1 << 16;
const PLACED_AIR: u32 = PLACED_BLOCK | object::air as u32;
const UNKNOWN: u32 = sentinel::unknown as u32;

/// The eye's windows' cells' zooms begin at the top's (pxl-kernel
/// layer/terrain/window.rs): its window at `cell` is `cell − FIRST_CELL`.
const FIRST_CELL: u32 = GROUND.cell_zoom(GROUND.top());

/// A cell's or a window's zoom, held to the eye's windows.
fn held_to(cell: u32) -> u32 {
    if cell < FIRST_CELL {
        FIRST_CELL
    } else if cell > LEAF_CELL {
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

/// The world as the GPU holds it (pxl-kernel scene/cast.rs fills it): the
/// ground's held tiles, the service area, the caves, and what is placed with
/// a preview's cuts. The ways' lines and decor are not here (pxl-portable
/// `cast::Query`); the ways' bound is, in each square's lane.
pub struct World<'a> {
    /// Per window of the eye's, from the top's, a word a slot: the square
    /// it holds, and whether it is present (pxl-portable `window`).
    window: &'a [IVec4],
    /// Per square of every window, its looks as handed, its cells' heights
    /// (`heights_cs`), their levels (`bound_cs`), and its tile's samples as
    /// handed (`window::SQUARE_WORDS`); the eye's windows first.
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
            let (cell, _) = ground::Held::cell(self, zoom, [n[0], n[1], c[2]]);
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
    fn cell_at(&self, cell: u32, g: [i32; 2]) -> (u32, i32, u32) {
        let (from, slot, i, here) = window::find(self, cell, g, FIRST_CELL);
        if !here {
            return (VOID, 0, cell);
        }
        (self.look(from, slot, i), self.height(from, slot, i), from)
    }

    /// The ground's cell `b` of `zoom` as the grid reads it, before the
    /// area's edge is laid: the cell drawing it, past the leaves a leaf's.
    fn ground_at(&self, zoom: u32, b: [i32; 2]) -> (u32, f32, [u32; 4]) {
        let read = held_to(zoom);
        let g = if zoom >= read {
            [b[0] >> (zoom - read), b[1] >> (zoom - read)]
        } else {
            [b[0] << (read - zoom), b[1] << (read - zoom)]
        };
        let (w, h, from) = self.cell_at(read, g);
        (ground::own(w, from), h as f32, ground::looks(w))
    }

    /// Whether the leaf cell `l` is gone: past the area, or nothing known
    /// of it, its look off the window drawing it.
    fn gone(&self, l: [i32; 2]) -> bool {
        let (w, _, from) = self.cell_at(LEAF_CELL, l);
        area::past(self, area::tile_at(LEAF_CELL, l)) || ground::own(w, from) == UNKNOWN
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

/// The ground as the GPU holds it, for pxl-portable's one answer for a node.
impl ground::Held for World<'_> {
    /// The join of the bounds over the node of each window the zooms
    /// `cells` read their cells from, the finest holding them present at
    /// each (`window::bounded`); none (`UNBOUNDED`) for a node wider than a
    /// square there, which only splits more.
    fn relief(&mut self, [coarsest, finest]: [u32; 2], (zoom, c): (u32, [i32; 3])) -> ([i32; 2], bool) {
        let (mut z, last) = (held_to(coarsest), held_to(finest));
        let mut relief = [0, 0];
        let mut first = true;
        while z <= last {
            let ([lo, hi], _, here) = window::bounded(self, z, (zoom, [c[0], c[1]]), FIRST_CELL);
            if !here {
                return ([0, 0], false);
            }
            relief = if first { [lo, hi] } else { [relief[0].min(lo), relief[1].max(hi)] };
            first = false;
            z += 1;
        }
        (relief, true)
    }

    /// The block's column off the tile drawing that zoom's cells, the area's
    /// edge laid on it (pxl-kernel query/node.rs `drawn`).
    fn cell(&mut self, zoom: u32, c: [i32; 3]) -> (ground::Cell, bool) {
        let b = [c[0], c[1]];
        let (look, h, fold) = self.ground_at(zoom, b);
        let gone = look == UNKNOWN;
        // Ground beside one gone: of the leaf cells its neighbours lie in
        // (`area::ring`), each read once, its look alone.
        let beside = !gone && zoom >= LEAF_CELL && Ground::grounds(look) && {
            let (ring, s) = (area::ring(zoom, b), zoom - LEAF_CELL);
            let mut any = false;
            let mut k = 0;
            while k < 9 {
                let l = [wrap_index(LEAF_CELL, (b[0] >> s) + (k % 3) as i32 - 1), (b[1] >> s) + (k / 3) as i32 - 1];
                any = any || (ring >> k & 1 == 1 && self.gone(l));
                k += 1;
            }
            any
        };
        let cell = ground::Cell {
            look: area::wall(look, zoom, gone, beside),
            top: ground::blocks(zoom, h),
            fold,
        };
        (cell, true)
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
        let i = ground::node(self, at.cave, (zoom, c), asked, block);
        let (value, f) = i.block;
        In {
            kind: i.kind,
            block: [value, f[0] & 0xff | (f[1] & 0xff) << 8 | (f[2] & 0xff) << 16 | (f[3] & 0xff) << 24],
        }
    }

    fn fills(&self, block: &[u32; 2]) -> bool {
        block[0] != VOID && block[0] != PLACED_AIR
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
    let base = square_at(at(fresh, 2 * id.z));
    let place = at(fresh, 2 * id.z + 1);
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
    let base = square_at(at(fresh, 2 * id.z));
    let w = {
        let under: &[u32] = ground;
        window::join(&Fresh { ground: under, base }, 0, 0, j, [id.x, id.y])
    };
    ground[(base + LEVELS_AT + window::level_at(j) + id.y * side + id.x) as usize] = w as u32;
}
