//! The cast's compute entry (`cast_cs`): each camera-grid cell's ray down
//! the pyramid by pxl-portable's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it; and the ground's grid it reads
//! (`heights_cs`), a held tile's cells' heights computed once. Each entry is
//! a module of its own. The harness's entries are behind its feature
//! (`harness`).

#![cfg_attr(target_arch = "spirv", no_std)]

#[cfg(feature = "harness")]
pub mod harness;

use pxl_portable::area::{self, Area};
use pxl_portable::cast::{self, Detail, Ends, In, Met, Query, Until};
use pxl_portable::caves::Cave;
use pxl_portable::cube::{sentinel, Ground};
use pxl_portable::ground::{self, Tile, ELE_PADDED_SPAN, EXTENT_LANE};
use pxl_portable::lattice::{EXTENT, GROUND, TILE_CELLS};
use pxl_portable::{wrap_index, LEAF_CELL};
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
}

const VOID: u32 = sentinel::void as u32;
const UNKNOWN: u32 = sentinel::unknown as u32;

/// Words a frame of looks holds: 4 a word.
const LOOK_WORDS: u32 = TILE_CELLS * TILE_CELLS / 4;
/// Words a frame of heights holds: 2 a word.
const ELE_WORDS: u32 = (ELE_PADDED_SPAN * ELE_PADDED_SPAN).div_ceil(2);
/// Cells a frame of the ground's grid holds.
const GRID_CELLS: u32 = TILE_CELLS * TILE_CELLS;
/// Words a frame of the index lane holds: its cells, then its coarser levels.
const INDEX_WORDS: u32 = EXTENT_LANE + EXTENT.coarse_cells();

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
        self.looks[(self.frame * LOOK_WORDS + i / 4) as usize] >> (8 * (i % 4)) & 0xff
    }
    fn height(&self, x: u32, y: u32) -> i32 {
        self.grid[(self.frame * GRID_CELLS + y * TILE_CELLS + x) as usize]
    }
}

/// The world as the GPU holds it (pxl-kernel scene/cast.rs fills it): the
/// ground's held tiles, the service area, and the caves. Ways, what is
/// placed and decor are not here (pxl-portable `cast::Query`).
pub struct World<'a> {
    /// Per ground zoom from the top, a header — the south-west tile, the
    /// tiles across — then a word a tile, its frame plus one, 0 where none
    /// is held.
    window: &'a [IVec4],
    looks: &'a [u32],
    /// Per frame, its cells' heights (`heights_cs`).
    grid: &'a [i32],
    extents: &'a [u32],
    /// The served shards: zoom, x, y.
    shards: &'a [IVec4],
}

impl Area for World<'_> {
    fn ends(&self) -> bool {
        true
    }
    fn shards(&self) -> u32 {
        self.shards.len() as u32
    }
    fn shard(&self, i: u32) -> (u32, [i32; 2]) {
        let s = self.shards[i as usize];
        (s.x as u32, [s.y, s.z])
    }
}

impl World<'_> {
    /// The word the window holds for the tile `t` at `zoom`: its frame plus
    /// one, 0 where none is held or the window does not reach it.
    fn held(&self, zoom: u32, t: [i32; 2]) -> u32 {
        let across = self.window[0].z as u32;
        let block = (zoom - GROUND.top()) * (1 + (across * across).div_ceil(4));
        let h = self.window[block as usize];
        let dx = (t[0] - h.x) as u32 & ((1u32 << zoom) - 1);
        let dy = t[1] - h.y;
        if dx >= across || dy < 0 || dy as u32 >= across {
            return 0;
        }
        let i = dy as u32 * across + dx;
        let w = self.window[(block + 1 + i / 4) as usize];
        [w.x, w.y, w.z, w.w][(i % 4) as usize] as u32
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

    /// The index lane's cell `i` of frame `f`: its lowest and highest.
    fn index(&self, f: u32, i: u32) -> [i32; 2] {
        let w = self.extents[(f * INDEX_WORDS + i) as usize];
        [half(w, 0), half(w, 16)]
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

    /// Whether the cell `b` of `zoom` is gone: past the area, or nothing
    /// known of it.
    fn gone(&self, read: u32, zoom: u32, b: [i32; 2]) -> bool {
        area::past(self, area::tile_at(zoom, b)) || self.ground_at(read, zoom, b, true).0 == UNKNOWN
    }
}

/// The ground as the GPU holds it, for pxl-portable's one answer for a node.
impl ground::Held for World<'_> {
    /// The index lane over the node, off the finest held tile over it at the
    /// tile zoom its cells at `cell` are read from.
    fn relief(&mut self, cell: u32, (zoom, c): (u32, [i32; 3])) -> ([i32; 2], bool) {
        let read = GROUND.read_zoom(cell);
        let s = zoom - read;
        let (from, w) = self.drawn(read, [c[0] >> s, c[1] >> s]);
        if w == 0 {
            return ([0, 0], false);
        }
        (ground::relief(|i| self.index(w - 1, i), from, (zoom, [c[0], c[1]])), true)
    }

    /// The block's column off the tile drawing that zoom's cells, the area's
    /// edge laid on it (pxl-kernel query/node.rs `drawn`).
    fn cell(&mut self, zoom: u32, c: [i32; 3]) -> (ground::Cell, bool) {
        let b = [c[0], c[1]];
        let read = GROUND.read_zoom(zoom);
        let (look, h, fold) = self.ground_at(read, zoom, b, false);
        let gone = look == UNKNOWN;
        let beside = !gone && zoom >= LEAF_CELL && Ground::grounds(look) && {
            let mut any = false;
            let mut k = 0;
            while k < 9 {
                let (dx, dy) = (k % 3 - 1, k / 3 - 1);
                any = any || (k != 4 && self.gone(read, zoom, [wrap_index(zoom, b[0] + dx), b[1] + dy]));
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

/// What a node is handed down on the GPU: its caves.
impl Query for World<'_> {
    type At = Cave;
    type Block = u32;
    const AIR: u32 = VOID;

    fn root(&mut self, node: [i32; 3]) -> Cave {
        Cave::of(0, node)
    }

    fn child(&mut self, cave: &Cave, zoom: u32, node: [i32; 3]) -> Cave {
        cave.child(zoom, node)
    }

    /// What is in the node `c` at `zoom`: the ground's one answer for a
    /// node (pxl-portable `ground::node`), asked at its own zoom at `near` or
    /// finer, or finer than the ground's cells, where the world holds nothing
    /// finer.
    fn node(&mut self, cave: &mut Cave, zoom: u32, c: [i32; 3], asked: [u32; 2], _t0: f32) -> In<u32> {
        let i = ground::node(self, *cave, (zoom, c), asked, zoom >= asked[1] || zoom >= LEAF_CELL);
        In { kind: i.kind, block: i.block.0 }
    }

    fn fills(&self, block: &u32) -> bool {
        *block != VOID
    }
}

/// The lane's cell for a ray: the value it stopped on, its kind (ground)
/// and the face it entered through, then how far as f32 bits, then the
/// nodes its descent asked about; where it met nothing, the first two zero.
fn word(ray: &cast::Ray, met: Met<Cave, u32>) -> UVec4 {
    if met.ends != Ends::Stop {
        return UVec4::new(0, 0, met.visits, 0);
    }
    let s = met.stop;
    let down = ray.direction[s.axis as usize] > 0.0;
    let face = 2 * s.axis + down as u32;
    UVec4::new(s.block | 1 << 16 | face << 18, s.t.to_bits(), met.visits, 0)
}

#[spirv(compute(threads(8, 8, 1)))]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grid: &[i32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] extents: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] lane: &mut [UVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] tangents: &[f32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] shards: &[IVec4],
) {
    let across = eye.across_range;
    if id.x >= across.x || id.y >= across.y {
        return;
    }
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let direction = cast::through(v3(eye.forward_far), v3(eye.right_tau), v3(eye.up), tangents[id.x as usize], tangents[(across.x + id.y) as usize]);
    let ray = cast::Ray::of([eye.origin.x, eye.origin.y, eye.origin.z], direction, eye.forward_far.w);
    let detail = Detail {
        tau: eye.right_tau.w,
        range: [across.z, across.w],
    };
    let mut world = World {
        window,
        looks,
        grid,
        extents,
        shards,
    };
    let met = cast::first(&mut world, &ray, detail, Until::Fills);
    lane[(1 + id.y * across.x + id.x) as usize] = word(&ray, met);
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
    grid[(frame * GRID_CELLS + id.y * TILE_CELLS + id.x) as usize] = h;
}
