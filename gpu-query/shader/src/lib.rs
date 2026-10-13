//! The cast's compute entry (`cast_cs`): each camera-grid cell's ray down
//! the pyramid by pxl-portable's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it. The harness's entries are behind its
//! feature (`harness`).

#![cfg_attr(target_arch = "spirv", no_std)]

#[cfg(feature = "harness")]
pub mod harness;

use pxl_portable::area::{self, Area};
use pxl_portable::cast::{self, Detail, Ends, In, Kind, Met, Query, Until};
use pxl_portable::caves::Cave;
use pxl_portable::cube::{sentinel, Ground};
use pxl_portable::ground::{self, Tile, ELE_PADDED_SPAN, EXTENT_LANE};
use pxl_portable::lattice::{EXTENT, GROUND, TILE_CELLS};
use pxl_portable::{wrap_index, LEAF_CELL, LEAF_ZOOM, SHARD_ZOOM};
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
/// Words a frame of the index lane holds: its cells, then its coarser levels.
const INDEX_WORDS: u32 = EXTENT_LANE + EXTENT.coarse_cells();

/// The low half of `w` shifted down by `s`, its sign carried.
fn half(w: u32, s: u32) -> i32 {
    ((w >> s) << 16) as i32 >> 16
}

/// One held tile's frame in the ground's buffers.
pub struct Framed<'a> {
    looks: &'a [u32],
    heights: &'a [u32],
    frame: u32,
}

impl Tile for Framed<'_> {
    fn look(&self, x: u32, y: u32) -> u32 {
        let i = y * TILE_CELLS + x;
        self.looks[(self.frame * LOOK_WORDS + i / 4) as usize] >> (8 * (i % 4)) & 0xff
    }
    fn ele(&self, i: usize) -> i32 {
        half(self.heights[(self.frame * ELE_WORDS) as usize + i / 2], 16 * (i as u32 % 2))
    }
}

/// The finest held tile on a node's line at or over a zoom: its zoom in the
/// high byte, its window word (frame plus one) under it; 0 where none is.
fn held_of((zoom, word): (u32, u32)) -> u32 {
    if word == 0 {
        0
    } else {
        zoom << 24 | word
    }
}

/// What a node is handed down on the GPU: its caves, and the finest held
/// tile on its line at or over its zoom (`held_of`), which its children
/// step from with one look at the window rather than a walk up it.
#[derive(Clone, Copy)]
pub struct Down {
    cave: Cave,
    held: u32,
}

/// What the walk reads of a column of cells at a zoom.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Read {
    /// Nothing read yet.
    None,
    /// The ground's cell.
    Cell,
    /// Past the service area: the Earth's.
    Earth,
    /// No tile held over it.
    Lacks,
}

/// The column of cells the walk read last, at its zoom: a ray passes a
/// column's blocks one after another, so each is handed the read rather
/// than reading it again.
#[derive(Clone, Copy)]
pub struct Column {
    zoom: u32,
    at: [i32; 2],
    read: Read,
    cell: ground::Cell,
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
    heights: &'a [u32],
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

    /// The finest held tile over the tile `t` at `zoom`, down to the top
    /// (`held_of`): for a tile beside a node's own, whose line the descent
    /// did not walk.
    fn beside(&self, zoom: u32, t: [i32; 2]) -> u32 {
        let mut z = zoom;
        loop {
            let s = zoom - z;
            let w = self.held(z, [t[0] >> s, t[1] >> s]);
            if w != 0 || z == GROUND.top() {
                return held_of((z, w));
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
    /// off the finest held tile over it: `own` where it lies in the tile
    /// `tile`, else the window's; its look and height, `void` where none is
    /// held; and whether only its look is asked.
    fn cell(&self, zoom: u32, g: [i32; 2], (tile, own): ([i32; 2], u32), only_look: bool) -> (u32, i32) {
        let shamt = GROUND.tile_cell_shamt();
        let t = [wrap_index(zoom, g[0] >> shamt), g[1] >> shamt];
        let held = if t[0] == tile[0] && t[1] == tile[1] { own } else { self.beside(zoom, t) };
        if held == 0 {
            return (VOID, 0);
        }
        let s = zoom - (held >> 24);
        // Its place from the held tile's first cell, in the asked tile's cells.
        let first = |v: i32| (v >> s) << (shamt + s);
        let mask = (1u32 << (zoom + shamt)) - 1;
        let v = [((g[0] - first(t[0])) as u32 & mask) as i32, g[1] - first(t[1])];
        let tile = Framed {
            looks: self.looks,
            heights: self.heights,
            frame: (held & 0xff_ffff) - 1,
        };
        if only_look {
            (ground::look(&tile, v, 1 << s), 0)
        } else {
            ground::read(&tile, v, 1 << s)
        }
    }

    /// The ground's cell `b` of `zoom` as the grid reads it, before the
    /// area's edge is laid: the tile cell drawing it, or the four it folds a
    /// zoom finer (`ground::fold`), off the tiles at `read`, `own` the held
    /// tile over its own; its look alone where `only_look`.
    fn ground_at(&self, read: u32, zoom: u32, b: [i32; 2], own: ([i32; 2], u32), only_look: bool) -> (u32, f32, [u32; 4]) {
        let k = GROUND.cell_zoom(read) as i32 - zoom as i32;
        // Tile cells it folds: four a zoom finer, else the one holding it.
        let (n, first) = if k == 1 { (4, [2 * b[0], 2 * b[1]]) } else { (1, [b[0] >> -k, b[1] >> -k]) };
        let mut four = [(VOID, 0); 4];
        let mut q = 0;
        while q < n {
            four[q as usize] = self.cell(read, [first[0] + (q & 1) as i32, first[1] + (q >> 1) as i32], own, only_look);
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
    fn gone(&self, read: u32, zoom: u32, b: [i32; 2], own: ([i32; 2], u32)) -> bool {
        area::past(self, area::tile_at(zoom, b)) || self.ground_at(read, zoom, b, own, true).0 == UNKNOWN
    }

    /// The column `b` of `zoom` as its blocks read it (pxl-kernel
    /// query/node.rs `drawn`): past the area, the Earth's; else the ground's
    /// cell off the tile drawing that zoom's cells, `own` the held tile over
    /// it, the area's edge laid on it.
    fn column(&self, zoom: u32, b: [i32; 2], own: ([i32; 2], u32)) -> Column {
        let mut column = Column {
            zoom,
            at: b,
            read: Read::Lacks,
            cell: ground::Cell { look: VOID, top: 0, fold: [VOID; 4] },
        };
        if area::past(self, area::tile_at(zoom, b)) {
            column.read = Read::Earth;
            return column;
        }
        if own.1 == 0 {
            return column;
        }
        let read = GROUND.read_zoom(zoom);
        let (look, h, fold) = self.ground_at(read, zoom, b, own, false);
        let gone = look == UNKNOWN;
        let beside = !gone && zoom >= LEAF_CELL && Ground::grounds(look) && {
            let mut any = false;
            let mut k = 0;
            while k < 9 {
                let (dx, dy) = (k % 3 - 1, k / 3 - 1);
                any = any || (k != 4 && self.gone(read, zoom, [wrap_index(zoom, b[0] + dx), b[1] + dy], own));
                k += 1;
            }
            any
        };
        column.read = Read::Cell;
        column.cell = ground::Cell {
            look: area::wall(look, zoom, gone, beside),
            top: ground::blocks(zoom, h),
            fold,
        };
        column
    }

    /// The held tile over the node `c` at `zoom`'s own tile at `read`: its
    /// ancestor's at that zoom, on `line`, or its own.
    fn own(line: &cast::Line<Down>, at: &Down, zoom: u32, read: u32, c: [i32; 3]) -> ([i32; 2], u32) {
        let s = zoom - read;
        ([c[0] >> s, c[1] >> s], if read == zoom { at.held } else { line[read as usize].held })
    }

    /// What is in the block `c` at `zoom` at its own zoom, its column read
    /// once (`column`) and its caves' value.
    fn block(&self, last: &mut Column, line: &cast::Line<Down>, at: &Down, zoom: u32, c: [i32; 3]) -> In<u32> {
        let b = [c[0], c[1]];
        if last.read == Read::None || last.zoom != zoom || last.at[0] != b[0] || last.at[1] != b[1] {
            let own = World::own(line, at, zoom, GROUND.read_zoom(zoom), c);
            *last = self.column(zoom, b, own);
        }
        match last.read {
            Read::Earth => In {
                kind: Kind::Block,
                block: area::earth(c[2]),
            },
            Read::Cell => In {
                kind: Kind::Block,
                block: last.cell.block(zoom, c[2], at.cave.hollow()).0,
            },
            _ => In { kind: Kind::Lacks, block: VOID },
        }
    }
}

impl Query for World<'_> {
    type At = Down;
    type Block = u32;
    const AIR: u32 = VOID;
    type Walk = Column;
    const WALK: Column = Column {
        zoom: 0,
        at: [0, 0],
        read: Read::None,
        cell: ground::Cell { look: VOID, top: 0, fold: [VOID; 4] },
    };

    fn root(&mut self, node: [i32; 3]) -> Down {
        Down { cave: Cave::of(0, node), held: 0 }
    }

    /// A child at a tile's zoom looks at the window for its own tile; past
    /// the leaves its parent's is its own.
    fn child(&mut self, at: &Down, zoom: u32, node: [i32; 3]) -> Down {
        let own = if zoom >= GROUND.top() && zoom <= LEAF_ZOOM {
            held_of((zoom, self.held(zoom, [node[0], node[1]])))
        } else {
            0
        };
        Down {
            cave: at.cave.child(zoom, node),
            held: if own != 0 { own } else { at.held },
        }
    }

    /// What is in the node `c` at `zoom` (pxl-kernel query/node.rs
    /// `World::node`): at `near` or finer, or finer than the ground's cells,
    /// where the world holds nothing finer, the block; coarser, past the area
    /// the Earth's, else the ground's one rule over the lowest and highest
    /// its held tile's index lane holds under it (`ground::empty`).
    fn node(&mut self, last: &mut Column, line: &cast::Line<Down>, at: &mut Down, (zoom, c): (u32, [i32; 3]), [far, near]: [u32; 2], _t0: f32) -> In<u32> {
        if zoom >= near || zoom >= LEAF_CELL {
            return self.block(last, line, at, zoom, c);
        }
        let split = In { kind: Kind::Split, block: VOID };
        let empty = In { kind: Kind::Block, block: VOID };
        if zoom >= SHARD_ZOOM && area::past(self, area::tile_at(zoom, [c[0], c[1]])) {
            return if area::earth(c[2]) == VOID { empty } else { split };
        }
        if zoom < GROUND.top() {
            return split;
        }
        let read = GROUND.read_zoom(near.min(GROUND.grid_zoom(zoom.min(LEAF_ZOOM))));
        let (_, held) = World::own(line, at, zoom, read, c);
        if held == 0 {
            return In { kind: Kind::Lacks, block: VOID };
        }
        let relief = ground::relief(|i| self.index((held & 0xff_ffff) - 1, i), held >> 24, (zoom, [c[0], c[1]]));
        if ground::empty(self, (zoom, c), relief, [far, near], at.cave.whole(zoom) == (true, true)) {
            empty
        } else {
            split
        }
    }

    fn fills(&self, block: &u32) -> bool {
        *block != VOID
    }
}

/// The lane's cell for a ray: the value it stopped on, its kind (ground)
/// and the face it entered through, then how far as f32 bits, then the
/// nodes its descent asked about; where it met nothing, the first two zero.
fn word(ray: &cast::Ray, met: Met<Down, u32>) -> UVec4 {
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] heights: &[u32],
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
        heights,
        extents,
        shards,
    };
    let met = cast::first(&mut world, &ray, detail, Until::Fills);
    lane[(1 + id.y * across.x + id.x) as usize] = word(&ray, met);
}
