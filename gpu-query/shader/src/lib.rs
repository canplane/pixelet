// gpu-query-shader/lib.rs

//! The cast's compute entry (`cast_cs`): each camera-grid cell's ray down
//! the pyramid by pxl-portable's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it; and the ground's grid it reads, a held
//! tile's cells' heights computed once (`heights_cs`) and their bound's
//! levels over them (`bound_cs`). Each entry is a module of its own. The harness's entries are behind its feature
//! (`harness`).

#![cfg_attr(target_arch = "spirv", no_std)]

#[cfg(feature = "harness")]
pub mod harness;

use pxl_portable::area::{self, Area};
use pxl_portable::cast::{self, Detail, Ends, In, Kind, Met, Query, Until};
use pxl_portable::caves::Cave;
use pxl_portable::cube::{object, sentinel, Ground};
use pxl_portable::ground::{self, Tile, ELE_PADDED_SPAN, GRID_WORDS};
use pxl_portable::lattice::{GROUND, TILE_CELLS};
use pxl_portable::placed::{self, Cut, Held, Lanes, BLOCK, PLACED, SPLIT};
use pxl_portable::{at, mask, wrap_index, LEAF_CELL, LEAF_ZOOM, SHARD_ZOOM};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
use spirv_std::spirv;

/// The eye a cast is from (pxl-kernel view/sight.rs `gpu_cast`).
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

/// Words a frame of looks holds: 4 a word.
const LOOK_WORDS: u32 = TILE_CELLS * TILE_CELLS / 4;
/// Words a frame of heights holds: 2 a word.
const ELE_WORDS: u32 = (ELE_PADDED_SPAN * ELE_PADDED_SPAN).div_ceil(2);

/// The low half of `w` shifted down by `s`, its sign carried.
fn half(w: u32, s: u32) -> i32 {
    ((w >> s) << 16) as i32 >> 16
}

/// Elevation sample `i` of frame `frame` of the tiles' heights.
fn ele(heights: &[u32], frame: u32, i: usize) -> i32 {
    half(heights[(frame * ELE_WORDS) as usize + i / 2], 16 * (i as u32 % 2))
}

/// One held tile's frame in the ground's buffers: its looks and its grid.
pub struct Framed<'a> {
    looks: &'a [u32],
    grid: &'a [i32],
    frame: u32,
}

impl Tile for Framed<'_> {
    fn look(&self, x: u32, y: u32) -> u32 {
        let i = y * TILE_CELLS + x;
        at(self.looks, self.frame * LOOK_WORDS + i / 4) >> (8 * (i % 4)) & 0xff
    }
    fn height(&self, x: u32, y: u32) -> i32 {
        at(self.grid, self.frame * GRID_WORDS + y * TILE_CELLS + x)
    }
}

/// The world as the GPU holds it (pxl-kernel scene/cast.rs fills it): the
/// ground's held tiles, the service area, the caves, and what is placed with
/// a preview's cuts. The ways and decor are not here (pxl-portable
/// `cast::Query`).
pub struct World<'a> {
    /// Per ground zoom from the top, a header — the south-west tile, the
    /// tiles across — then a word a tile, its frame plus one, 0 where none
    /// is held.
    window: &'a [IVec4],
    looks: &'a [u32],
    /// Per frame, its cells' heights (`heights_cs`), then their bound's
    /// levels (`bound_cs`).
    grid: &'a [i32],
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

    /// The word the window holds for the tile `t` at `zoom`: its frame plus
    /// one, 0 where none is held or the window does not reach it.
    fn held(&self, zoom: u32, t: [i32; 2]) -> u32 {
        let across = at(self.window, 0).z as u32;
        let block = (zoom - GROUND.top()) * (1 + (across * across).div_ceil(4));
        let h = at(self.window, block);
        let dx = (t[0] - h.x) as u32 & ((1u32 << zoom) - 1);
        let dy = t[1] - h.y;
        if dx >= across || dy < 0 || dy as u32 >= across {
            return 0;
        }
        let i = dy as u32 * across + dx;
        let w = at(self.window, block + 1 + i / 4);
        (match i % 4 {
            0 => w.x,
            1 => w.y,
            2 => w.z,
            _ => w.w,
        }) as u32
    }

    /// The finest held tile over the tile `t` at `zoom`, down to the top: its
    /// zoom and its window word, 0 where none is.
    fn drawn(&self, zoom: u32, t: [i32; 2]) -> (u32, u32) {
        let mut z = zoom;
        loop {
            let s = zoom - z;
            let w = self.held(z, [t[0] >> s, t[1] >> s]);
            if w != 0 || z == GROUND.top() {
                return (z, w);
            }
            z -= 1;
        }
    }

    /// The ground's cell `g` of the tiles at `zoom`, counted in their cells,
    /// off the finest held tile over it: its look and height, `void` where
    /// none is held; and whether only its look is asked.
    fn cell(&self, zoom: u32, g: [i32; 2], only_look: bool) -> (u32, i32) {
        let shamt = GROUND.tile_cell_shamt();
        let t = [wrap_index(zoom, g[0] >> shamt), g[1] >> shamt];
        let (from, w) = self.drawn(zoom, t);
        if w == 0 {
            return (VOID, 0);
        }
        let s = zoom - from;
        // Its place from the held tile's first cell, in the asked tile's cells.
        let first = |v: i32| (v >> s) << (shamt + s);
        let mask = (1u32 << (zoom + shamt)) - 1;
        let v = [((g[0] - first(t[0])) as u32 & mask) as i32, g[1] - first(t[1])];
        let tile = Framed {
            looks: self.looks,
            grid: self.grid,
            frame: w - 1,
        };
        if only_look {
            (ground::look(&tile, v, 1 << s), 0)
        } else {
            ground::read(&tile, v, 1 << s)
        }
    }

    /// The ground's cell `b` of `zoom` as the grid reads it, before the
    /// area's edge is laid: the tile cell drawing it, or the four it folds a
    /// zoom finer (`ground::fold`), off the tiles at `read`; its look alone
    /// where `only_look`.
    fn ground_at(&self, read: u32, zoom: u32, b: [i32; 2], only_look: bool) -> (u32, f32, [u32; 4]) {
        let k = GROUND.cell_zoom(read) as i32 - zoom as i32;
        // Tile cells it folds: four a zoom finer, else the one holding it.
        let (n, first) = if k == 1 { (4, [2 * b[0], 2 * b[1]]) } else { (1, [b[0] >> -k, b[1] >> -k]) };
        let mut four = [(VOID, 0); 4];
        let mut q = 0;
        while q < n {
            four[q as usize] = self.cell(read, [first[0] + (q & 1) as i32, first[1] + (q >> 1) as i32], only_look);
            q += 1;
        }
        if n == 4 {
            ground::fold(four, zoom)
        } else {
            let (look, h) = four[0];
            (look, h as f32, [look; 4])
        }
    }

    /// Whether the leaf cell `l` is gone: past the area, or nothing known
    /// of it, its look off the leaf tile drawing it.
    fn gone(&self, l: [i32; 2]) -> bool {
        area::past(self, area::tile_at(LEAF_CELL, l)) || self.cell(LEAF_ZOOM, l, true).0 == UNKNOWN
    }
}

/// The ground as the GPU holds it, for pxl-portable's one answer for a node.
impl ground::Held for World<'_> {
    /// The join of the bounds over the node of each tile the zooms `cells`
    /// read their cells from, the finest held over it at each, off its grid
    /// (`ground::bounded`), which bounds that tile's own cells alone; none
    /// (`UNBOUNDED`) for a node wider than the finest asked's tile, which
    /// holds more than one.
    fn relief(&mut self, [coarsest, finest]: [u32; 2], (zoom, c): (u32, [i32; 3])) -> ([i32; 2], bool) {
        if GROUND.read_zoom(finest) > zoom {
            return (ground::UNBOUNDED, true);
        }
        let mut relief = [0, 0];
        let mut z = coarsest;
        while z <= finest {
            let read = GROUND.read_zoom(z);
            if z == coarsest || read != GROUND.read_zoom(z - 1) {
                let s = zoom - read;
                let (from, w) = self.drawn(read, [c[0] >> s, c[1] >> s]);
                if w == 0 {
                    return ([0, 0], false);
                }
                let (grid, base) = (self.grid, (w - 1) * GRID_WORDS);
                let [lo, hi] = ground::bounded(move |i| at(grid, base + i), from, (zoom, [c[0], c[1]]));
                relief = if z == coarsest { [lo, hi] } else { [relief[0].min(lo), relief[1].max(hi)] };
            }
            z += 1;
        }
        (relief, true)
    }

    /// The block's column off the tile drawing that zoom's cells, the area's
    /// edge laid on it (pxl-kernel query/node.rs `drawn`).
    fn cell(&mut self, zoom: u32, c: [i32; 3]) -> (ground::Cell, bool) {
        let b = [c[0], c[1]];
        let read = GROUND.read_zoom(zoom);
        let (look, h, fold) = self.ground_at(read, zoom, b, false);
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grid: &[i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] lane: &mut [UVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] tangents: &[f32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] overwrites: &[u32],
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
        looks,
        grid,
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

/// The ground's grid of the frames `fresh` names, the frames uploaded this
/// turn, one a `z`: each cell's height (`ground::height`) of the tile held
/// there, at its own cells.
#[spirv(compute(threads(8, 8, 1)))]
pub fn heights_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] heights: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grid: &mut [i32],
) {
    let frame = fresh[id.z as usize];
    let h = ground::height(move |i| ele(heights, frame, i), [id.x, id.y]);
    grid[(frame * GRID_WORDS + id.y * TILE_CELLS + id.x) as usize] = h;
}

/// Level `level.x` of the bound of the frames `fresh` names, one a `z`,
/// over the level under it, written before (`ground::join`): a dispatch a
/// level, from 1 up, after `heights_cs`.
#[spirv(compute(threads(8, 8, 1)))]
pub fn bound_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] level: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grid: &mut [i32],
) {
    let j = level.x;
    let side = TILE_CELLS >> j;
    if id.x >= side || id.y >= side {
        return;
    }
    let base = fresh[id.z as usize] * GRID_WORDS;
    let w = {
        let under: &[i32] = grid;
        ground::join(move |i| under[(base + i) as usize], j, [id.x, id.y])
    };
    grid[(base + ground::bound_at(j) + id.y * side + id.x) as usize] = w;
}
