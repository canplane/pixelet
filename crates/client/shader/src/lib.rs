// pxl-shader/lib.rs

//! The cast's compute entry (`cast_cs`): each drawn pixel's ray down the
//! pyramid by pxl-spec-core's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it; and the ground of each square the eye's
//! windows hold, filled once as it is handed: its columns' tops off its
//! tile's samples (`heights_cs`), its ways' runs off its lines (`ways_cs`)
//! and its levels (`bound_cs`), which the CPU reads back; and the decor the
//! ground grows in the eye's windows (`decor_cs`). Each entry is a
//! module of its own.

#![cfg_attr(target_arch = "spirv", no_std)]
// The workspace's clippy never builds this crate: its own build refuses a
// warning, as `-D warnings` does every other.
#![deny(warnings)]

use pxl_spec_core::area::Area;
use pxl_spec_core::answer::{self, In};
use pxl_spec_core::cast::{self, Detail, Ends, Met, Query, Until};
use pxl_spec_core::decor::{Decor, COUNTS};
use pxl_spec_core::eye;
use pxl_spec_core::fill::{self, column_of, level_of, put};
use pxl_spec_core::ways::tree::{self, Pool};
use pxl_spec_core::ground;
use pxl_spec_core::lattice::GROUND;
use pxl_spec_core::overwrite::{self, Cut, Lanes};
use pxl_spec_core::ways;
use pxl_spec_core::scatter::{self, drawn, Grounds, SQUARE_BLOCKS, SQUARE_GROWN};
use pxl_spec_core::window::{self, Squares, FIRST_CELL, SLOTS, SQUARE, SQUARE_CELLS, SQUARE_SHIFT, SQUARE_WORDS};
use pxl_spec_core::window::Found;
use pxl_spec_core::world::At;
use pxl_spec_core::{at, beam, faces, observed, ray, world, LEAF_CELL};
use spirv_std::glam::{IVec4, UVec2, UVec3, UVec4, Vec2, Vec4};
use spirv_std::arch::IndexUnchecked;
use spirv_std::memory::{Scope, Semantics};
use spirv_std::{spirv, Image};

/// The eye a cast is from (pxl-kernel view/lens.rs `gpu_cast`), its words
/// where pxl-spec-core `eye` lays them (asserted below).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Eye {
    /// The Q3 position, and whether a ray reports the nodes its descent
    /// asked (`word`), not what it met.
    pub origin: IVec4,
    /// The unit forward, and the reach.
    pub forward_far: Vec4,
    /// The unit right, and τ.
    pub right_tau: Vec4,
    /// The unit up, and a pad.
    pub up: Vec4,
    /// The tangents of the screen's half-angles across and up.
    pub tan: Vec2,
    /// The zooms a block is asked at. [naga] A vec2 of u32s, which naga's
    /// SPIR-V reader looks for, and never makes, where it splits the layer
    /// off an arrayed image's coordinate (`Rays`).
    pub range: UVec2,
    /// A preview's cut to air and its left-out leaf: X, Y, Z and the zoom,
    /// `eye::NO_CUT` for none.
    pub carve: IVec4,
    pub hide: IVec4,
    /// The camera grid the frame casts on, as the kernel laid it once
    /// (pxl-spec-core `beam::Grid`, `grid`).
    pub grid: [UVec4; eye::GRID_WORDS / 4],
    /// The sun's cast, a frame's constants (pxl-spec-core `sun::Sun`), all
    /// zero where no shadow is cast.
    pub sun: [Vec4; 5],
}

/// Word `words`'s byte.
const fn at_word(words: usize) -> usize {
    words * core::mem::size_of::<u32>()
}

const _: () = {
    use at_word as at;
    use core::mem::{offset_of, size_of};
    assert!(size_of::<Eye>() == at(eye::WORDS));
    assert!(offset_of!(Eye, origin) == at(eye::ORIGIN) && offset_of!(Eye, forward_far) == at(eye::FORWARD) && offset_of!(Eye, right_tau) == at(eye::RIGHT));
    assert!(offset_of!(Eye, up) == at(eye::UP) && offset_of!(Eye, tan) == at(eye::TAN) && offset_of!(Eye, range) == at(eye::RANGE));
    assert!(offset_of!(Eye, carve) == at(eye::CARVE) && offset_of!(Eye, hide) == at(eye::HIDE) && offset_of!(Eye, grid) == at(eye::GRID) && offset_of!(Eye, sun) == at(eye::SUN));
};

/// A preview's entry as pxl-spec-core `overwrite` takes it.
fn cut(e: IVec4) -> Cut {
    Cut {
        zoom: e.w as u32,
        at: [e.x, e.y, e.z],
    }
}


/// Square `square`'s first word in the ground's buffer (pxl-spec-core
/// `window::SQUARE_WORDS`).
fn square_at(square: u32) -> u32 {
    square * SQUARE_WORDS
}

/// The world as the GPU holds it (pxl-kernel scene/cast.rs fills it): the
/// squares the eye's windows hold, their columns, levels and runs, the
/// service area, and what is placed with a preview's cuts; the caves are a
/// hash, held nowhere. Decor is not here (pxl-spec-core `cast::Query`).
/// The squares' cells' four looks are `L`'s: their own buffer where the
/// cast draws by them, none where a pass draws nothing (`Unlooked`).
#[derive(Clone, Copy)]
pub struct World<'a, L> {
    /// Per window of the eye's, from the top's, a row a slot: the square it
    /// holds, and whether it is present (pxl-spec-core `window`).
    window: &'a [IVec4],
    /// Per square of every window, its columns (`heights_cs`, `ways_cs`),
    /// their levels (`bound_cs`), their runs (`ways_cs`) and its cells as
    /// laid (`heights_cs`) (`window::SQUARE_WORDS`); the eye's windows first.
    ground: &'a [u32],
    /// Per square, in the same order, its cells' four looks (`Looks`).
    looks: L,
    /// The service area's index (pxl-spec-core `area::Area`), its keys four a
    /// row.
    shards: &'a [IVec4],
    /// The overwrites' tree (pxl-kernel layer/object/store.rs): its heap
    /// lane, then its data lane from half the buffer.
    overwrites: &'a [u32],
    /// The eye, whose preview cuts are read where something placed or cut
    /// is asked about, not held through the descent.
    eye: &'a Eye,
    /// The ways' pool (pxl-spec-core `ways::tree`), every square's tree,
    /// which every pass that asks the world's query reads.
    pool: ways::tree::Read<'a>,
}

/// Where the squares' cells' four looks are read: a word a cell
/// (`ground::looks`), `window::SQUARE_CELLS` a square in the squares' order,
/// as the fill wrote them off the handed record (`heights_cs`).
pub trait Looks: Copy {
    fn look(&self, i: u32) -> u32;
}

impl Looks for &[u32] {
    fn look(&self, i: u32) -> u32 {
        at(self, i)
    }
}

/// No looks bound: a pass that draws nothing reads none, every cell's four
/// the word it holds, none (`ground::word`'s zero). A word, not nothing:
/// rust-gpu offsets no pointer to a field of no size.
#[derive(Clone, Copy)]
pub struct Unlooked(u32);

impl Looks for Unlooked {
    fn look(&self, _: u32) -> u32 {
        self.0
    }
}

impl<L: Looks> Lanes for World<'_, L> {
    fn heap(&self, slot: u32) -> u32 {
        at(self.overwrites, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        at(self.overwrites, self.overwrites.len() as u32 / 2 + slot)
    }
}

impl<L: Looks> Area for World<'_, L> {
    fn ends(&self) -> bool {
        true
    }
    /// Its cells' nines four a row, by key (pxl-kernel scene/cast.rs
    /// `shards`); none past them.
    fn nine(&self, key: u32) -> u32 {
        let r = at(self.shards, key / 4);
        (match key % 4 {
            0 => r.x,
            1 => r.y,
            2 => r.z,
            _ => r.w,
        }) as u32
    }
}

impl<L> World<'_, L> {
    /// Row `r` of the windows' table (`window::Slot`).
    fn row(&self, r: u32) -> window::Slot {
        let h = at(self.window, r);
        window::Slot::of([h.x, h.y, h.z, h.w])
    }
}

/// The eye's windows, as the GPU holds them.
impl<L: Looks> Squares for World<'_, L> {
    /// The slot square `q` is held in, while it holds it present.
    fn present(&self, cell: u32, q: [i32; 2]) -> (u32, bool) {
        let s = window::slot(q);
        (s, self.row((cell - FIRST_CELL) * SLOTS + s).holds(q))
    }
    fn square(&self, cell: u32, slot: u32) -> ([i32; 2], bool) {
        self.row((cell - FIRST_CELL) * SLOTS + slot).held()
    }
    /// Off the windows' rows past their table (`joined_cs`).
    fn joined(&self, cell: u32, e: u32) -> [u32; window::JOINED_WORDS as usize] {
        let r = at(self.window, window::JOINED_AT + (cell - FIRST_CELL) * window::JOINED_ENTRIES + e);
        [r.x as u32, r.y as u32, r.z as u32, r.w as u32]
    }
    /// The window's row past every window's slots (`window::ORIGIN_AT`).
    fn origin(&self, cell: u32) -> ([i32; 2], bool) {
        self.row(window::ORIGIN_AT + (cell - FIRST_CELL)).held()
    }
    fn look(&self, cell: u32, slot: u32, i: u32) -> u32 {
        self.looks.look(((cell - FIRST_CELL) * SLOTS + slot) * SQUARE_CELLS + i)
    }
    fn column(&self, cell: u32, slot: u32, i: u32) -> u32 {
        column_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), i)
    }
    fn level(&self, cell: u32, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        level_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), j, at)
    }
    fn laid(&self, cell: u32, slot: u32, i: u32) -> u32 {
        fill::laid_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot), i)
    }
    fn root(&self, cell: u32, slot: u32) -> u32 {
        fill::root_of(self.ground, square_at((cell - FIRST_CELL) * SLOTS + slot))
    }
}

/// The ground as the GPU holds it, for pxl-spec-core's one answer for a node.
impl<L: Looks> ground::Held for World<'_, L> {
    /// The bound over the node of the window `cell` reads its cells from,
    /// the finest holding them present (`window::bound_over`), as the CPU reads
    /// it (pxl-spec-core `Held::relief`).
    fn relief(&mut self, place: &mut Found, cells: [u32; 2], (zoom, c): (u32, [i32; 3])) -> (ground::Bounds, bool) {
        let (r, _, here) = window::bound_over(self, place, cells, (zoom, [c[0], c[1]]));
        (r, here)
    }

    /// The block's column off the windows, the area's edge laid on it
    /// (pxl-spec-core `window::ground`), as the CPU reads it (pxl-kernel
    /// query/node.rs `Reading::cell`).
    fn cell(&mut self, place: Found, zoom: u32, c: [i32; 3]) -> ground::Cell {
        window::ground(self, place, zoom, [c[0], c[1]]).0
    }
}

/// What a node is on the GPU: its value, one of the catalog's whichever
/// source holds it, and the four looks the ground's block is drawn by
/// (`world::node`'s second word), what the screen paints a hit with.
impl<L: Looks> Query for World<'_, L> {
    type At = At;

    fn root(&mut self, node: [i32; 3]) -> At {
        At::of(self, (0, node))
    }

    fn child(&mut self, at: &At, zoom: u32, node: [i32; 3]) -> At {
        at.child(self, &self.pool, (zoom, node))
    }

    /// What is in the node `c` at `zoom`: the world's one query
    /// (pxl-spec-core `world::node`).
    fn node(&mut self, at: &mut At, zoom: u32, c: [i32; 3], asked: [u32; 2], _t0: f32) -> In<[u32; 2]> {
        let (w, cuts) = (*self, [cut(self.eye.carve), cut(self.eye.hide)]);
        world::node(self, &w, &w, &w.pool, at, (zoom, c), asked, cuts)
    }
}

/// The world as decor's growth asks it on the GPU (pxl-spec-core `scatter`):
/// the cast's, and the catalog's rows as handed.
struct Growing<'a> {
    world: World<'a, Unlooked>,
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

/// The world's ray down a column and a block, each the cast's world's one
/// query.
impl Grounds for Growing<'_> {
    fn rows(&self) -> &[Decor] {
        self.rows
    }

    /// The world's ray down the column, the cast's own descent
    /// (pxl-spec-core `scatter::footing`).
    fn footing(&mut self, cell: u32, g: [i32; 2]) -> (u32, (i32, f32), bool) {
        scatter::footing(&mut self.world, cell, g)
    }

    fn block(&mut self, zoom: u32, c: [i32; 3]) -> u32 {
        self.world.block(zoom, c)
    }
}

impl<L: Looks> World<'_, L> {
    /// The value of the block `c` at `zoom`, asked of the world's one
    /// query with nothing handed down (pxl-spec-core `world::At::of`).
    fn block(&mut self, zoom: u32, c: [i32; 3]) -> u32 {
        let mut at = At::of(self, (zoom, c));
        self.node(&mut at, zoom, c, [zoom, zoom], 0.0).block[0]
    }

    /// What lies about the face a ray stopped on (pxl-spec-core
    /// `faces::word`): each block beside the stop's at its zoom
    /// (`World::block`), filling its cube whole (`answer::whole`); a box's
    /// own, a block kept short of its cube (`faces::BOX`); nothing where the
    /// ray stopped on none.
    fn faced<A: Copy>(&mut self, origin: [i32; 3], ray: &cast::Ray, met: &Met<A>) -> u32 {
        if met.ends != Ends::Stop {
            return 0;
        }
        if answer::kept(met.stop.block[0]) < answer::EIGHTHS {
            return faces::BOX;
        }
        let (zoom, c) = (met.stop.node.zoom, met.stop.node.index(origin));
        faces::word(face_of(ray, met.stop.axis), |d| answer::whole(self.block(zoom, faces::beside(zoom, c, d))))
    }
}

/// The face a ray enters a block through across `axis` (pxl-spec-core
/// `ray`): 2·axis, plus one where its outward normal points down the axis,
/// against the ray.
fn face_of(ray: &cast::Ray, axis: u32) -> u32 {
    2 * axis + (pxl_spec_core::axis(ray.direction, axis) > 0.0) as u32
}

/// A ray's words, a layer each of `Rays`: the value it stopped on and the
/// face it entered through, then how far as f32 bits, then the four looks
/// the ground's block is drawn by, a byte each (`ground::word`), or in the
/// steps view the nodes its descent asked about, then what lies about its
/// face (`World::faced`); where it met nothing, the first two zero.
fn word<A: Copy>(ray: &cast::Ray, met: Met<A>, steps: bool, faces: u32) -> [u32; ray::WORDS as usize] {
    if met.ends != Ends::Stop {
        return [0, 0, met.visits, 0];
    }
    let s = met.stop;
    let [value, four] = s.block;
    [ray::first_word(value, face_of(ray, s.axis), s.node.zoom), s.t.to_bits(), if steps { met.visits } else { four }, faces]
}
const _: () = assert!(ray::FACES == 3, "what lies about a ray's face is its last word");

/// Where the eye's rays start, and the zooms each distance asks.
fn eye_origin(eye: &Eye) -> ([i32; 3], Detail) {
    (
        [eye.origin.x, eye.origin.y, eye.origin.z],
        Detail {
            tau: eye.right_tau.w,
            range: [eye.range.x, eye.range.y],
            eye: [0.0; 3],
        },
    )
}

/// The tangents across and up of the middle of drawn pixel `p` of the
/// `size` the rays are, row 0 at the top, as the screen pass forms its own
/// pixel's: the screen's half-angles' scaled by where it lies.
fn pixel_tan(eye: &Eye, p: UVec3, size: UVec3) -> [f32; 2] {
    let ndc_x = (p.x as f32 + 0.5) / size.x as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p.y as f32 + 0.5) / size.y as f32 * 2.0;
    [ndc_x * eye.tan.x, ndc_y * eye.tan.y]
}

/// The eye's ray at the tangents `[tx, ty]`, reaching as far as the eye.
fn eye_ray(eye: &Eye, [tx, ty]: [f32; 2]) -> cast::Ray {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    cast::Ray::new(cast::through(v3(eye.forward_far), v3(eye.right_tau), v3(eye.up), tx, ty), eye.forward_far.w)
}

/// The camera grid the frame casts on, read whole off the eye's words, as
/// the kernel worked it out once (pxl-spec-core `beam::Grid`): no thread
/// works any of it out again.
fn grid(eye: &Eye) -> beam::Grid {
    let g = &eye.grid;
    let w = [
        g[0].x, g[0].y, g[0].z, g[0].w, g[1].x, g[1].y, g[1].z, g[1].w, g[2].x, g[2].y, g[2].z, g[2].w, g[3].x, g[3].y, g[3].z, g[3].w,
    ];
    beam::Grid::of_words([eye.tan.x, eye.tan.y], w)
}
const _: () = assert!(eye::GRID_WORDS == 16, "`grid` reads the grid's four vec4s");

/// The observed pyramid's buffer (pxl-spec-core `observed`), bound as a
/// runtime array, whose pointer a walk holds whole: a slice's would be a
/// pointer and a length, which naga traces no atomic through.
type Seen = spirv_std::RuntimeArray<u32>;

/// The beam rays' buffer (pxl-spec-core `beam`): the lines, the cells'
/// starts and the pixels the cast missed, appended by one atomic add, so
/// a runtime array as `Seen` is. The pixels' cast reads the pyramid alone
/// and appends to this, so the pyramid's binding stays read-only there and
/// naga types no word of it atomic in the walk's loop.
type Beams = spirv_std::RuntimeArray<u32>;

/// Word `i` of the observed pyramid's buffer.
fn seen_at(w: &Seen, i: u32) -> u32 {
    // SAFETY: an entry's or the head's word, within the buffer.
    unsafe { *w.index(i as usize) }
}

fn seen_put(w: &mut Seen, i: u32, v: u32) {
    // SAFETY: as `seen_at`.
    unsafe { *w.index_mut(i as usize) = v }
}

/// Word `i` read and written whole, where a walk may race another on it.
fn seen_load(w: &mut Seen, i: u32) -> u32 {
    // SAFETY: as `seen_at`.
    unsafe { spirv_std::arch::atomic_load::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(w.index_mut(i as usize)) }
}

fn seen_store(w: &mut Seen, i: u32, v: u32) {
    // SAFETY: as `seen_at`.
    unsafe { spirv_std::arch::atomic_store::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(w.index_mut(i as usize), v) }
}

fn seen_add(w: &mut Seen, i: u32, v: u32) -> u32 {
    // SAFETY: as `seen_at`.
    unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(w.index_mut(i as usize), v) }
}

/// Word `i` set to `v` where it holds `was`; what it held.
fn seen_swap(w: &mut Seen, i: u32, was: u32, v: u32) -> u32 {
    // SAFETY: as `seen_at`.
    unsafe { spirv_std::arch::atomic_compare_exchange::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }, { Semantics::NONE.bits() }>(w.index_mut(i as usize), v, was) }
}

/// The observed pyramid as a pixel's walk reads it, writing nothing
/// (pxl-spec-core `observed`): a node it keeps at the zooms asked is its
/// entry's answer; any other is a miss, which ends the walk as a lack would,
/// its block `MISSED`, which no lack's is. A miss is told by the answer and
/// not by a field: a walk's state that changes in its loop holds no pointer,
/// which naga takes no struct holding.
struct Reading<'a> {
    seen: &'a Seen,
}

/// The block a miss ends a walk on.
const MISSED: [u32; 2] = [observed::NONE, observed::NONE];

impl Query for Reading<'_> {
    type At = u32;

    fn root(&mut self, node: [i32; 3]) -> u32 {
        if observed::keeps(node) {
            observed::octant(node)
        } else {
            observed::NONE
        }
    }

    fn child(&mut self, parent: &u32, _: u32, node: [i32; 3]) -> u32 {
        if *parent == observed::NONE {
            return observed::NONE;
        }
        let c = seen_at(self.seen, observed::entry_at(*parent) + observed::W_CHILDREN);
        if c == 0 || c == observed::CLAIMED {
            observed::NONE
        } else {
            observed::child(c - 1, observed::octant(node))
        }
    }

    fn node(&mut self, e: &mut u32, _: u32, _: [i32; 3], asked: [u32; 2], _: f32) -> In<[u32; 2]> {
        if *e != observed::NONE {
            let w = observed::entry_at(*e);
            if observed::holds(seen_at(self.seen, w), asked) {
                let kind = observed::kind_of(seen_at(self.seen, w + observed::W_KIND));
                return In { kind, block: [seen_at(self.seen, w + observed::W_BLOCK), seen_at(self.seen, w + observed::W_BLOCK + 1)] };
            }
        }
        In { kind: pxl_spec_core::answer::Kind::Lacks, block: MISSED }
    }
}

/// Where a writing walk is: the node's entry, `observed::NONE` where none
/// keeps it, and what was handed it.
#[derive(Clone, Copy)]
struct Writ {
    entry: u32,
    at: At,
}

/// The observed pyramid as a beam ray's walk, or a pixel's that missed,
/// reads it: a node it keeps at the zooms asked, as its entry holds it,
/// unless this pass is still writing it (`tag`); any other asked of the
/// world's one query there and then, from what was handed it, and written.
/// A node's children are taken a tile once, by whichever walk swaps it in
/// first, off the tiles' stack.
struct Writing<'a, L> {
    world: World<'a, L>,
    seen: &'a mut Seen,
    tag: u32,
}

impl<L: Looks> Writing<'_, L> {
    /// The entry of `parent`'s child `oct`, its tile taken where it has
    /// none: claimed first by one compare-and-swap, so one walk of the many
    /// that reach a node at once takes a tile, and the rest walk on under
    /// it with none kept; none where no tile is left, one taken too many
    /// given back at once, as no tile is given back while walks take them.
    fn tile(&mut self, parent: u32, oct: u32) -> u32 {
        let w = observed::entry_at(parent) + observed::W_CHILDREN;
        let c = seen_load(self.seen, w);
        if c == observed::CLAIMED {
            return observed::NONE;
        }
        if c != 0 {
            return observed::child(c - 1, oct);
        }
        let was = seen_swap(self.seen, w, 0, observed::CLAIMED);
        if was != 0 {
            return if was == observed::CLAIMED { observed::NONE } else { observed::child(was - 1, oct) };
        }
        let k = seen_add(self.seen, observed::TAKEN, 1);
        if k >= observed::TILES {
            seen_add(self.seen, observed::TAKEN, 1u32.wrapping_neg());
            seen_store(self.seen, w, 0);
            return observed::NONE;
        }
        let t = seen_at(self.seen, observed::STACK_AT + k);
        seen_store(self.seen, w, t + 1);
        observed::child(t, oct)
    }
}

impl<L: Looks> Query for Writing<'_, L> {
    type At = Writ;

    fn root(&mut self, node: [i32; 3]) -> Writ {
        let entry = if observed::keeps(node) { observed::octant(node) } else { observed::NONE };
        Writ { entry, at: Query::root(&mut self.world, node) }
    }

    fn child(&mut self, parent: &Writ, zoom: u32, node: [i32; 3]) -> Writ {
        let entry = if parent.entry == observed::NONE { observed::NONE } else { self.tile(parent.entry, observed::octant(node)) };
        Writ { entry, at: Query::child(&mut self.world, &parent.at, zoom, node) }
    }

    fn node(&mut self, n: &mut Writ, zoom: u32, c: [i32; 3], asked: [u32; 2], t0: f32) -> In<[u32; 2]> {
        let w = observed::entry_at(n.entry);
        if n.entry != observed::NONE {
            let key = seen_load(self.seen, w);
            if observed::holds(key, asked) && observed::tag_of(key) != self.tag {
                let a = w + observed::W_AT;
                n.at = At::of_words([seen_at(self.seen, a), seen_at(self.seen, a + 1), seen_at(self.seen, a + 2), seen_at(self.seen, a + 3), seen_at(self.seen, a + 4)]);
                let kind = observed::kind_of(seen_at(self.seen, w + observed::W_KIND));
                return In { kind, block: [seen_at(self.seen, w + observed::W_BLOCK), seen_at(self.seen, w + observed::W_BLOCK + 1)] };
            }
        }
        let answer = Query::node(&mut self.world, &mut n.at, zoom, c, asked, t0);
        if n.entry != observed::NONE {
            seen_put(self.seen, w + observed::W_KIND, observed::kind_word(answer.kind));
            seen_put(self.seen, w + observed::W_BLOCK, answer.block[0]);
            seen_put(self.seen, w + observed::W_BLOCK + 1, answer.block[1]);
            let words = n.at.words();
            let a = w + observed::W_AT;
            seen_put(self.seen, a, words[0]);
            seen_put(self.seen, a + 1, words[1]);
            seen_put(self.seen, a + 2, words[2]);
            seen_put(self.seen, a + 3, words[3]);
            seen_put(self.seen, a + 4, words[4]);
            seen_put(self.seen, w + observed::W_NODE, zoom);
            seen_put(self.seen, w + observed::W_NODE + 1, c[0] as u32);
            seen_put(self.seen, w + observed::W_NODE + 2, c[1] as u32);
            seen_store(self.seen, w, observed::key(self.tag, asked));
        }
        answer
    }
}

/// The beam rays' lines (pxl-spec-core `beam::LINE_WORDS` a corner) as a
/// walk resumes from one: the entry of each node of the line at `base`, and
/// what was handed it (`world::At::words`), its own at its zoom.
#[derive(Clone, Copy)]
struct Beamed<'a>(&'a Beams, u32);

impl Beamed<'_> {
    /// The node the corner's beam ray stopped at.
    fn node(self) -> cast::Node {
        node_at(self.0, self.1 + beam::LINE_NODE)
    }

    /// The first word a zoom of the line holds.
    fn step(self, zoom: u32) -> u32 {
        self.1 + beam::LINE_AT + beam::LINE_STEP * zoom
    }
}

impl cast::Over<u32> for Beamed<'_> {
    fn over(&self, zoom: u32) -> u32 {
        seen_at(self.0, self.step(zoom))
    }
}

impl cast::Over<Writ> for Beamed<'_> {
    fn over(&self, zoom: u32) -> Writ {
        let (w, k) = (self.0, self.step(zoom));
        Writ {
            entry: seen_at(w, k),
            at: At::of_words([seen_at(w, k + 1), seen_at(w, k + 2), seen_at(w, k + 3), seen_at(w, k + 4), seen_at(w, k + 5)]),
        }
    }
}

/// The node at word `k` of the beam rays' buffer, as a line and a sample
/// each hold one (pxl-spec-core `beam::NODE_WORDS`).
fn node_at(w: &Beams, k: u32) -> cast::Node {
    cast::Node {
        corner: [seen_at(w, k + beam::NODE_CORNER), seen_at(w, k + beam::NODE_CORNER + 1), seen_at(w, k + beam::NODE_CORNER + 2)],
        zoom: seen_at(w, k + beam::NODE_ZOOM),
        roots: [seen_at(w, k + beam::NODE_ROOTS) as i32, seen_at(w, k + beam::NODE_ROOTS + 1) as i32, seen_at(w, k + beam::NODE_ROOTS + 2) as i32],
    }
}

/// Node `n` put at word `k` of the beam rays' buffer, as `node_at` reads it.
fn node_put(w: &mut Beams, k: u32, n: &cast::Node) {
    seen_put(w, k + beam::NODE_ZOOM, n.zoom);
    seen_put(w, k + beam::NODE_CORNER, n.corner[0]);
    seen_put(w, k + beam::NODE_CORNER + 1, n.corner[1]);
    seen_put(w, k + beam::NODE_CORNER + 2, n.corner[2]);
    seen_put(w, k + beam::NODE_ROOTS, n.roots[0] as u32);
    seen_put(w, k + beam::NODE_ROOTS + 1, n.roots[1] as u32);
    seen_put(w, k + beam::NODE_ROOTS + 2, n.roots[2] as u32);
}

/// Where the ray at the screen's tangents `tan` starts from the screen's
/// cell `cell` of `grid`: that cell's least, the corner's line it resumes
/// from, and the ray (pxl-spec-core `beam`).
fn started<'a>(eye: &Eye, grid: &beam::Grid, beams: &'a Beams, cell: [u32; 2], tan: [f32; 2]) -> (cast::Ray, f32, Beamed<'a>) {
    let at = grid.cell_at(cell);
    let t = f32::from_bits(seen_at(beams, at + beam::CELL_T));
    (eye_ray(eye, tan), t, Beamed(beams, beam::Grid::line(seen_at(beams, at + beam::CELL_CORNER))))
}

/// `ray`'s walk to the block that fills, resumed at `t` from the deepest
/// node of `line` holding where it is there, or, where that lies in another
/// root, from the root at it (doc/minutes/20261022-a-cell-casts-a-beam.md,
/// "Where a pixel resumes"); through `q`, reading or writing.
fn resumed<'a, Q: Query>(q: &mut Q, origin: [i32; 3], ray: &cast::Ray, detail: Detail, t: f32, line: &Beamed<'a>) -> Met<Q::At>
where
    Beamed<'a>: cast::Over<Q::At>,
{
    let (h, held) = line.node().over(cast::point_at(origin, ray, t));
    let h = cast::holding(origin, ray, h, t);
    let from = if held { (h, <Beamed<'a> as cast::Over<Q::At>>::over(line, h.zoom), t) } else { (cast::Node::ROOT, q.root(cast::Node::ROOT.index(origin)), t) };
    cast::walk(q, origin, ray, detail, Until::Fills, from, line, &mut ())
}

/// A ray's words written into the rays at pixel `p`.
fn written(rays: &Rays, p: UVec3, w: [u32; ray::WORDS as usize]) {
    let mut k = 0;
    while k < ray::WORDS {
        // SAFETY: within the rays, bounded by their size where `p` was.
        unsafe { rays.write(UVec3::new(p.x, p.y, k), w[k as usize]) };
        k += 1;
    }
}

/// Each beam ray of the camera grid, a thread a corner (pxl-spec-core
/// `beam::Grid::corners`): its walk from the world's root to the first block that
/// fills or lacks, or the first split too narrow to share
/// (`cast::Until::Beam`), through the observed pyramid writing what it asks
/// (`Writing`), and its line there, for the pixels about it.
#[spirv(compute(threads(8, 8, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    if id.x >= g.corners[0] || id.y >= g.corners[1] {
        return;
    }
    let ray = eye_ray(eye, g.corner([id.x, id.y]));
    let tag = observed::tag(seen_at(seen, observed::FRAME), 0);
    let world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let base = beam::Grid::line(g.corner_index([id.x, id.y]));
    let met = cast::rooted(&mut Writing { world, seen, tag }, origin, &ray, detail, Until::Beam, &mut Leaving(beams, base));
    seen_put(beams, base + beam::LINE_T, g.handed(met.stop.t).to_bits());
}

/// A beam ray's line as it leaves it (pxl-spec-core `cast::Keep`), into the
/// beam rays' words at `base` (`beam::LINE_WORDS` a corner), as `Beamed`
/// reads it.
struct Leaving<'a>(&'a mut Beams, u32);

impl cast::Keep<Writ> for Leaving<'_> {
    fn keep(&mut self, n: &cast::Node, live: Writ, stack: &[Writ; cast::DEPTH]) {
        let base = self.1;
        let beams = &mut *self.0;
        node_put(beams, base + beam::LINE_NODE, n);
        let mut z = 0;
        while z <= n.zoom {
            let held = if z == n.zoom { live } else { stack[z as usize] };
            let w = held.at.words();
            let k = base + beam::LINE_AT + beam::LINE_STEP * z;
            seen_put(beams, k, held.entry);
            seen_put(beams, k + 1, w[0]);
            seen_put(beams, k + 2, w[1]);
            seen_put(beams, k + 3, w[2]);
            seen_put(beams, k + 4, w[3]);
            seen_put(beams, k + 5, w[4]);
            z += 1;
        }
    }
}

/// Where each cell's pixels start (pxl-spec-core `beam::CELL_WORDS`), a
/// thread a cell of the screen (`beam::Grid::span`): the least of the beam
/// rays within its own window of its corners (`Grid::cell_window`), every
/// one cast since the widest window is the grid's `past`, and that corner,
/// whose line they resume from; past the lines in the beam rays' buffer
/// (`Grid::cell_at`).
#[spirv(compute(threads(8, 8, 1)))]
pub fn cells_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
) {
    let g = grid(eye);
    if id.x >= g.span[0] || id.y >= g.span[1] {
        return;
    }
    let k = g.cell_window([id.x, id.y]);
    let [x, y] = g.first([id.x, id.y]);
    let t_of = |c: u32| f32::from_bits(seen_at(beams, beam::Grid::line(c) + beam::LINE_T));
    let first = g.corner_index([x - k, y - k]);
    let (mut least, mut from) = (t_of(first), first);
    let mut j = y - k;
    while j <= y + 1 + k {
        let mut i = x - k;
        while i <= x + 1 + k {
            let c = g.corner_index([i, j]);
            let t = t_of(c);
            if t < least {
                (least, from) = (t, c);
            }
            i += 1;
        }
        j += 1;
    }
    let cell = g.cell_at([id.x, id.y]);
    seen_put(beams, cell + beam::CELL_T, least.to_bits());
    seen_put(beams, cell + beam::CELL_CORNER, from);
}

/// Each sample of the screen (pxl-spec-core `beam::Grid::samples`), a thread each:
/// its ray resumed from its cell's start as a pixel's is, walked to the
/// block that fills through the observed pyramid writing what it asks
/// (`Writing`), and the node and block it stopped at left for the pixels
/// about it (`beam::SAMPLE_WORDS`), `beam::SAMPLE_NONE` where it went on
/// to its reach, whether what it stopped at lacks, and what lies about the
/// face it entered (`World::faced`), asked here once for every pixel that
/// takes the block.
#[spirv(compute(threads(8, 8, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn sample_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    if id.x >= g.samples[0] || id.y >= g.samples[1] {
        return;
    }
    let (ray, t, line) = started(eye, &g, beams, g.sample_cell([id.x, id.y]), g.sample([id.x, id.y]));
    let tag = observed::tag(seen_at(seen, observed::FRAME), 1);
    let mut world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let met = resumed(&mut Writing { world, seen, tag }, origin, &ray, detail, t, &line);
    let faces = world.faced(origin, &ray, &met);
    let base = g.sample_at([id.x, id.y]);
    let stop = met.stop;
    node_put(beams, base + beam::SAMPLE_NODE, &stop.node);
    if met.ends == Ends::On {
        seen_put(beams, base + beam::SAMPLE_NODE + beam::NODE_ZOOM, beam::SAMPLE_NONE);
    }
    seen_put(beams, base + beam::SAMPLE_LACKS, (met.ends == Ends::Lacks) as u32);
    seen_put(beams, base + beam::SAMPLE_BLOCK, stop.block[0]);
    seen_put(beams, base + beam::SAMPLE_BLOCK + 1, stop.block[1]);
    seen_put(beams, base + beam::SAMPLE_AXIS, stop.axis);
    seen_put(beams, base + beam::SAMPLE_FACES, faces);
}

/// The eye's rays, a drawn pixel each, a layer a word of the ray's (`word`;
/// pxl-spec-core `ray`): the cast writes them.
type Rays = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// Each pixel's ray: the nearest of the blocks the samples about it stopped
/// at that it enters (pxl-spec-core `beam::Grid::about`, `cast::entry`), its
/// words, no step its own; where it enters none, or a node a sample lacks
/// at before it, the ray from its cell's start, reading the
/// observed pyramid alone (`Reading`), its binding read-only so no read of
/// it is atomic: its words where every node it asked was kept, else its
/// first word the mark `miss_cs` walks it again by (pxl-spec-core
/// `ray::MISS`). A block a sample stopped at comes with what lies about
/// its face, the sample's; a pixel that walked itself asks the world's
/// query its own (`World::faced`).
#[spirv(compute(threads(8, 8, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    // Read-write as the group lays them out, which WebGPU holds a shader's
    // own access to; no atomic touches them here, so their reads stay
    // plain.
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let (seen, beams, window): (&Seen, &Beams, &[IVec4]) = (seen, beams, window);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let (ray, t, line) = started(eye, &g, beams, g.cell(px), px);
    let about = g.about(px);
    // The nearest block entered, at the reach where none is.
    let mut hit = cast::Stop {
        t: ray.reach,
        axis: 0,
        node: cast::Node::ROOT,
        at: 0u32,
        block: MISSED,
    };
    // What lies about its face, the sample's.
    let mut faces = 0;
    // The nearest node a sample lacks at that the ray enters.
    let mut lack = ray.reach;
    let mut j = 0;
    while j < 2 * beam::NEAR {
        let mut i = 0;
        while i < 2 * beam::NEAR {
            let k = g.sample_at([about[0] + i, about[1] + j]);
            if seen_at(beams, k + beam::SAMPLE_NODE + beam::NODE_ZOOM) != beam::SAMPLE_NONE {
                let node = node_at(beams, k + beam::SAMPLE_NODE);
                let block = [seen_at(beams, k + beam::SAMPLE_BLOCK), seen_at(beams, k + beam::SAMPLE_BLOCK + 1)];
                let (enter, axis, enters) = cast::entry(origin, &ray, &node, block[0]);
                let lacks = seen_at(beams, k + beam::SAMPLE_LACKS) != 0;
                // A block's face the pixel enters across another axis than its
                // sample did lies under what the sample passed: not the
                // pixel's first, so it is no candidate.
                let exposed = axis == seen_at(beams, k + beam::SAMPLE_AXIS);
                if enters && lacks && enter < lack {
                    lack = enter;
                } else if enters && !lacks && exposed && enter < hit.t {
                    hit = cast::Stop {
                        t: enter,
                        axis,
                        node,
                        at: 0u32,
                        block,
                    };
                    faces = seen_at(beams, k + beam::SAMPLE_FACES);
                }
            }
            i += 1;
        }
        j += 1;
    }
    let steps = eye.origin.w != 0;
    if hit.t < ray.reach && hit.t < lack {
        written(rays, id, word(&ray, Met { ends: Ends::Stop, stop: hit, visits: 0 }, steps, faces));
        return;
    }
    let met = resumed(&mut Reading { seen }, origin, &ray, detail, t, &line);
    if met.ends == Ends::Lacks && met.stop.block[0] == MISSED[0] {
        // SAFETY: within the rays, bounded by their size above.
        unsafe { rays.write(UVec3::new(id.x, id.y, 0), ray::MISS) };
    } else {
        let mut world = World {
            window,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        };
        written(rays, id, word(&ray, met, steps, world.faced(origin, &ray, &met)));
    }
}

/// The world's depth from the sun, a slot of the sun's clipmap each
/// (pxl-spec-core `sun`): the slot's texel of the light box, and its ray
/// from the box's near plane along the sun's rays, walked through the
/// world's one query to the block that fills (`cast::first`) as the eye's
/// are, each node asked at the zoom the eye's rays ask it, and the word it
/// leaves there; none walked where the slot held its world texel last cast
/// and the eye asks its stop as it did (`Sun::kept`, `Sun::stands`), its
/// ray passing over no node the world's query answers otherwise since
/// (`moved`, `sun::over`). Its one layer read and written in place, the one
/// format family WebGPU lets a shader do both to, as the rays' are.
type SunWords = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The nodes the world's query answers otherwise than when the sun's map
/// was last cast (pxl-spec-core `sun::MOVED_WORDS`): how many and the
/// coarsest zoom the eye asks within the light box (`Sun::farthest`), then
/// each X, Y, its zoom, and 1 for a preview's cut, whose top bounds
/// nothing (`moved_top`), else 0.
#[repr(C)]
pub struct SunMoved {
    pub len: UVec4,
    pub at: [IVec4; pxl_spec_core::sun::MOVED],
}
const _: () = assert!(core::mem::size_of::<SunMoved>() == at_word(pxl_spec_core::sun::MOVED_WORDS));
const MOVED: usize = pxl_spec_core::sun::MOVED;

/// The sun's cast off the eye's words (pxl-spec-core `sun::Sun::words`).
fn sun_of(eye: &Eye) -> pxl_spec_core::sun::Sun {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let i = |v: Vec4| v.w.to_bits() as i32;
    pxl_spec_core::sun::Sun {
        first: v3(eye.sun[0]),
        reach: eye.sun[0].w,
        across: v3(eye.sun[1]),
        down: v3(eye.sun[2]),
        dir: v3(eye.sun[3]),
        bias: v3(eye.sun[4]),
        corner: [i(eye.sun[1]), i(eye.sun[2])],
        kept: [i(eye.sun[3]), i(eye.sun[4])],
    }
}

/// Slot `slot` of the clipmap `size` a side: its texel's ray, from where
/// and at what zooms, and whether it walks again, where its world texel
/// was not held, the eye asks its stop at another zoom, or its ray passes
/// over a node moved where it may meet what the node holds now, `tops`
/// how high that reaches (`moved_top`, pxl-spec-core `sun::below`).
fn sun_slot(eye: &Eye, sun: &SunWords, moved: &SunMoved, tops: &[i32; MOVED], size: u32, slot: UVec3) -> (bool, [i32; 3], cast::Ray, Detail) {
    let s = sun_of(eye);
    let p = s.texel(size, [slot.x, slot.y]);
    let (origin, mut detail) = eye_origin(eye);
    let (o, ray, from) = pxl_spec_core::sun::ray(origin, &s, p);
    detail.eye = from;
    let mut walks = true;
    if s.kept(size, p) {
        let held: u32 = sun.read(slot);
        if s.stands(detail, held) {
            let t = pxl_spec_core::sun::depth_of(held) * s.reach;
            let lacked = pxl_spec_core::sun::Held::NONE.with(held).lacked;
            let mut k = 0;
            while k < moved.len.x {
                let n = moved.at[k as usize];
                let top = if lacked { pxl_spec_core::sun::NO_TOP } else { tops[k as usize] };
                let from = pxl_spec_core::sun::below(o[2], 0.0, s.dir[2], top, t - s.pushed());
                if pxl_spec_core::sun::over_any(o, [[0.0; 2]; 2], s.dir, [from, t], [n.x, n.y], n.z as u32) {
                    break;
                }
                k += 1;
            }
            walks = k < moved.len.x;
        }
    }
    (walks, o, ray, detail)
}

/// How high what moved node `n` holds now may reach, as a Q3 z: the
/// ground's and the ways' bound over it at every zoom the eye asks within
/// the light box, from `far` (`SunMoved`) to its finest
/// (`window::bound_over`), no lower than the sea, rounded up as each of
/// those zooms' blocks round it (`ground::slab`); none (`sun::NO_TOP`) for
/// a preview's cut, or where no window holds it.
fn moved_top<L: Looks>(world: &World<'_, L>, eye: &Eye, far: u32, n: IVec4) -> i32 {
    if n.w != 0 {
        return pxl_spec_core::sun::NO_TOP;
    }
    let zoom = n.z as u32;
    let near = eye.range.y;
    let far = if far < near { far } else { near };
    let first = ground::coarsest(zoom, far);
    let cells = [if first > near { near } else { first }, near];
    let mut place = Found::NONE;
    let (b, _, here) = window::bound_over(world, &mut place, cells, (zoom, [n.x, n.y]));
    if !here {
        return pxl_spec_core::sun::NO_TOP;
    }
    let hi = if b[0][1] > b[1][1] { b[0][1] } else { b[1][1] };
    let hi = if hi > pxl_spec_core::SEA_LEVEL { hi } else { pxl_spec_core::SEA_LEVEL };
    let over = ground::slab(pxl_spec_core::area::Shard::UNKNOWN, (zoom, [n.x, n.y, 0]), [b[0][0], hi], [far, near], false)[1];
    pxl_spec_core::sun::top_of(if over > hi { over } else { hi })
}

/// Each moved node's top (`moved_top`), a lane a node, into the
/// workgroup's `tops` every lane reads past the barrier.
fn moved_tops<L: Looks>(world: &World<'_, L>, eye: &Eye, moved: &SunMoved, lane: u32, tops: &mut [i32; MOVED]) {
    if lane < moved.len.x && lane < MOVED as u32 {
        // SAFETY: both bounded by `MOVED` just above, and unchecked so no
        // lane leaves before the barrier, which they all reach.
        unsafe { *tops.index_unchecked_mut(lane as usize) = moved_top(world, eye, moved.len.y, *moved.at.index_unchecked(lane as usize)) };
    }
    spirv_std::arch::workgroup_memory_barrier_with_group_sync();
}

/// The sun's clipmap's tiles a slot of which may walk again, listed
/// (pxl-spec-core `sun::TILE`): a thread a tile, judged whole — whether its
/// run of box texels was all kept, whether the eye asks the zoom its stops
/// were cast at wherever in their box it looks (`Sun::tile_stands`), and
/// whether its rays together pass over a node moved (`sun::over_any`) —
/// and the tile appended where it may, its row of the walk's dispatch with
/// it; the walk judges each slot of a listed tile itself (`sun_slot`). A
/// slot's own judgement for every slot of every tile, its rays against
/// every node moved, cost the shadows tens of milliseconds a moving frame.
#[spirv(compute(threads(64, 1, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn sun_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(local_invocation_index)] lane: u32,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &SunMoved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(workgroup)] tops: &mut [i32; MOVED],
) {
    let world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    moved_tops(&world, eye, moved, lane, tops);
    let size: UVec3 = sun.query_size();
    let t = pxl_spec_core::sun::TILE;
    let across = size.x / t;
    if id.x >= across * across {
        return;
    }
    let tile = [id.x % across, id.x / across];
    let s = sun_of(eye);
    let (p, run) = s.tile_texel(size.x, tile);
    let mut walks = !run || !s.tile_kept(size.x, p);
    if !walks {
        // Its slots' words, as one: the stops they hold and the zoom they
        // were asked at, judged where they lie as a box.
        let mut held = pxl_spec_core::sun::Held::NONE;
        let mut k = 0;
        while k < t * t {
            let word: u32 = sun.read(UVec3::new(tile[0] * t + k % t, tile[1] * t + k / t, 0));
            held = held.with(word);
            k += 1;
        }
        let (origin, detail) = eye_origin(eye);
        walks = !s.tile_stands(detail, p, held);
        if !walks && moved.len.x != 0 {
            // Its rays as one: the lowest start, the nearest stop and the
            // deepest reach (pxl-spec-core `sun::below`).
            let (o, off) = s.tile_rays(origin, p);
            let share = s.reach / pxl_spec_core::sun::DEPTH_ONE as f32;
            let (reach, near) = (held.reach as f32 * share, held.near as f32 * share - s.pushed());
            let drop = s.tile_drop();
            let mut n = 0;
            while n < moved.len.x {
                let m = moved.at[n as usize];
                let top = if held.lacked { pxl_spec_core::sun::NO_TOP } else { tops[n as usize] };
                let from = pxl_spec_core::sun::below(o[2], drop, s.dir[2], top, near);
                if pxl_spec_core::sun::over_any(o, off, s.dir, [from, reach], [m.x, m.y], m.z as u32) {
                    break;
                }
                n += 1;
            }
            walks = n < moved.len.x;
        }
    }
    if walks {
        let at = add(list, pxl_spec_core::sun::LIST_COUNT as u32, 1);
        let row = at / pxl_spec_core::sun::LIST_ACROSS + 1;
        if pxl_spec_core::sun::LIST_ROWS < list.len() {
            // SAFETY: just bounded by the slice's own length.
            unsafe { spirv_std::arch::atomic_u_max::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(list.get_unchecked_mut(pxl_spec_core::sun::LIST_ROWS), row) };
        }
        put(list, pxl_spec_core::sun::LIST_AT as u32 + at, tile[1] << 16 | tile[0]);
    }
}

const _: () = assert!(pxl_spec_core::sun::TILE == 8, "`sun_cs` is a workgroup a tile, its threads a side of it");

/// The world's depth from the sun into the slots the mark found walk again
/// (`sun_mark_cs`): a workgroup a listed tile, a thread a slot, each walked
/// through the world's one query to the block that fills (`cast::first`)
/// as the eye's rays are, and the word it leaves there.
#[spirv(compute(threads(8, 8, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn sun_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &SunMoved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &[u32],
    #[spirv(workgroup)] tops: &mut [i32; MOVED],
) {
    let window: &[IVec4] = window;
    let mut world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let t = pxl_spec_core::sun::TILE;
    // Before any lane leaves: the barrier is every lane's.
    moved_tops(&world, eye, moved, lane.y * t + lane.x, tops);
    let size: UVec3 = sun.query_size();
    let k = group.y * pxl_spec_core::sun::LIST_ACROSS + group.x;
    if k >= at(list, pxl_spec_core::sun::LIST_COUNT as u32) {
        return;
    }
    let tile = at(list, pxl_spec_core::sun::LIST_AT as u32 + k);
    let slot = UVec3::new((tile & 0xffff) * t + lane.x, (tile >> 16) * t + lane.y, 0);
    if slot.x >= size.x || slot.y >= size.y {
        return;
    }
    let (walks, o, ray, detail) = sun_slot(eye, sun, moved, tops, size.x, slot);
    if !walks {
        return;
    }
    let s = sun_of(eye);
    let met = cast::first(&mut world, o, &ray, detail, Until::Fills);
    // SAFETY: within the map, bounded by its size above.
    unsafe { sun.write(slot, s.word_of(detail, &met)) };
}

/// Each pixel `cast_cs` marked missed, its ray again from its cell's
/// start, writing what it asks into the observed pyramid (`Writing`), and
/// what lies about the face it stopped on (`World::faced`): a thread a
/// drawn pixel, as the cast's, each but the marked returning at once. It
/// runs after `cast_cs` in the cast's one pass and writes every marked
/// pixel's words whole, so the mark never outlives the pass.
#[spirv(compute(threads(8, 8, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn miss_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let (window, beams): (&[IVec4], &Beams) = (window, beams);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first != ray::MISS {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let (ray, t, line) = started(eye, &g, beams, g.cell(px), px);
    let tag = observed::tag(seen_at(seen, observed::FRAME), 2);
    let mut world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let met = resumed(&mut Writing { world, seen, tag }, origin, &ray, detail, t, &line);
    written(rays, id, word(&ray, met, eye.origin.w != 0, world.faced(origin, &ray, &met)));
}

/// What the observed pyramid kept forgotten, a thread an entry, where what
/// the world's query reads changed (pxl-spec-core `observed::FORGETS`): every
/// entry's key and its children's tile, every tile on the stack in turn; or
/// the key of each entry over a square that moved (`observed::forgets`), its
/// children kept for the walk that asks it again, and its children's tile
/// given back once that keeps nothing. This dispatch only clears words, so
/// a child read before its own thread cleared it keeps its tile a
/// forgetting longer, and one read cleared is cleared.
#[spirv(compute(threads(64, 1, 1)))]
pub fn forget_cs(#[spirv(global_invocation_id)] id: UVec3, #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen) {
    if id.x >= observed::ENTRIES {
        return;
    }
    let w = observed::entry_at(id.x);
    if seen_at(seen, observed::FORGETS) == observed::FORGETS_ALL {
        seen_put(seen, w + observed::W_KEY, 0);
        seen_put(seen, w + observed::W_CHILDREN, 0);
        if id.x < observed::TILES {
            seen_put(seen, observed::STACK_AT + id.x, id.x);
        }
        return;
    }
    if seen_load(seen, w + observed::W_KEY) != 0 {
        let (zoom, x, y) = (seen_at(seen, w + observed::W_NODE), seen_at(seen, w + observed::W_NODE + 1) as i32, seen_at(seen, w + observed::W_NODE + 2) as i32);
        let mut moved = [0; observed::MOVED_WORDS as usize];
        let mut k = 0;
        while k < observed::MOVED_WORDS {
            moved[k as usize] = seen_at(seen, observed::MOVED_AT + k);
            k += 1;
        }
        if observed::forgets(&moved, zoom, [x, y]) {
            seen_store(seen, w + observed::W_KEY, 0);
        }
    }
    let c = seen_load(seen, w + observed::W_CHILDREN);
    if c == 0 {
        return;
    }
    let mut kept = false;
    let mut oct = 0;
    while oct < 8 {
        let e = observed::entry_at(observed::child(c - 1, oct));
        kept = kept || seen_load(seen, e + observed::W_KEY) != 0 || seen_load(seen, e + observed::W_CHILDREN) != 0;
        oct += 1;
    }
    if !kept {
        seen_store(seen, w + observed::W_CHILDREN, 0);
        // One fewer taken: an add of -1, as the count's word is unsigned.
        let k = seen_add(seen, observed::TAKEN, 1u32.wrapping_neg());
        seen_put(seen, observed::STACK_AT + k - 1, c - 1);
    }
}

/// The eye's windows folded over their squares, a thread an entry
/// (pxl-spec-core `window::fold`), each a row of the windows' buffer past
/// their slot table (`window::JOINED_AT`): each window's levels over a
/// square, the join of its aligned blocks of squares, which a node wider
/// than a square reads as one entry (`window::bound_over`), a level a dispatch
/// off the one under it. Run where the windows' slots moved; a present
/// square's levels stand while it does.
#[spirv(compute(threads(64, 1, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn joined_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(num_workgroups)] groups: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
) {
    // The level this dispatch folds is its depth in workgroups, its last
    // slice the one that runs: the levels under it were folded before.
    let (l, blocks) = (groups.z, (window::ACROSS >> groups.z) * (window::ACROSS >> groups.z));
    if id.z + 1 != l || id.x >= blocks * window::EYE_WINDOWS {
        return;
    }
    let (cell, b) = (FIRST_CELL + id.x / blocks, id.x % blocks);
    let [x, y, bound, pad] = {
        let read: &[IVec4] = window;
        let world = World {
            window: read,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        };
        window::fold(&world, cell, l, b)
    };
    let k = (window::JOINED_AT + (cell - FIRST_CELL) * window::JOINED_ENTRIES + window::joined_index(l, [0, 0]) + b) as usize;
    if k < window.len() {
        window[k] = IVec4::new(x as i32, y as i32, bound as i32, pad as i32);
    }
}

/// The columns' ground tops of the squares `fresh` names, the squares
/// handed their tiles' samples this turn, one a `z`, each a record as
/// handed (`window::HANDED_WORDS`); a thread a column (`ground::height`)
/// off the samples in it, an even column's laying its cell and the next
/// (`fill::heights`); and its cell's four looks into their own buffer
/// (`fill::look`), which the cast draws by.
#[spirv(compute(threads(8, 8, 1)))]
pub fn heights_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] looks: &mut [u32],
) {
    if id.x >= SQUARE || id.y >= SQUARE {
        return;
    }
    fill::heights(ground, fresh, id.z, [id.x, id.y]);
    fill::look(looks, fresh, id.z, [id.x, id.y]);
}

/// Level `level.x` of the squares `fresh` names, one a `z`: level 0 each
/// column's own, its top and its runs in its scratch (`fill::level`), a
/// higher one over the level under it, written before (`window::join`): a
/// dispatch a level, from 0 up.
#[spirv(compute(threads(8, 8, 1)))]
pub fn bound_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] level: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
    // Read and written, as `ways_cs` binds it: the two share one group.
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] scratch: &mut [u32],
) {
    let j = level.x;
    let side = SQUARE >> j;
    if id.x >= side || id.y >= side {
        return;
    }
    let rec = window::handed_at(id.z);
    let base = square_at(at(fresh, rec + window::FRESH_SQUARE));
    let runs = fill::Scratch {
        words: &*scratch,
        at: fill::scratch_at(id.z),
    };
    fill::level(ground, base, &runs, at(fresh, rec + window::FRESH_CELL), j, [id.x, id.y]);
}

/// The ways of the squares `fresh` names, one a `z`, a thread a column
/// (`x`, cells row by row): each column's spans and those beside it off the
/// square's lines (pxl-spec-core `ways::around`) read along z as runs
/// (`ways::runs`), into the square's scratch from its column's first
/// (`window::FIRSTS_AT`). Three dispatches, `pass.x` 0 to 2: each column
/// counts its runs into its first; a workgroup a square sums them into each
/// column's first, each
/// thread a stretch (`fill::stretch_runs`), the stretches' sums scanned in
/// the workgroup's memory (Hillis and Steele 1986; GPU Gems 3, ch. 39) and
/// each stretch's firsts written off it (`fill::scanned`); each column
/// writes its runs from its first. Over `heights_cs`'s tops: a way is told
/// from their faces. Each square's entry as handed whole: where it lies
/// and its shard lay the area's edge (pxl-spec-core
/// `ways::Columns::walled`).
#[spirv(compute(threads(64, 1, 1)))]
pub fn ways_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] pass: &UVec4,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] fresh: &[u32],
    // Read and written, as `bound_cs` binds it: the two share one group.
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] scratch: &mut [u32],
    #[spirv(workgroup)] sums: &mut [u32; fill::SCAN_THREADS as usize],
) {
    if pass.x != 1 {
        fill::ways(ground, fresh, scratch, id.z, pass.x == 2, id.x);
        return;
    }
    // An inclusive scan, a step a doubling: each sum adds the one `d` before
    // it, both read before any is written. Each index is masked to the
    // array's length and read unchecked (spirv-std `IndexUnchecked`), so no
    // bounds check branches on the lane between the barriers, which WebGPU
    // takes only in uniform control flow; and two loads lie between them, a
    // lane's own, so they are no one load every lane shares
    // (pxl-shader-build's `workgroupUniformLoad`).
    let t = lane.x;
    let own = fill::stretch_runs(scratch, id.z, t);
    fill::scanned(scratch, id.z, t, scan(sums, t, own) - own);
}

/// The workgroup's words, a lane's own: a scan's sums, a level's numbers.
type Shared = [u32; fill::SCAN_THREADS as usize];

/// Thread `t`'s inclusive sum of the workgroup's `own`s, a step a doubling
/// (Hillis and Steele 1986; GPU Gems 3, ch. 39): each sum adds the one `d`
/// before it, both read before any is written. Each index is masked to the
/// array's length and read unchecked (spirv-std `IndexUnchecked`), so no
/// bounds check branches on the lane between the barriers, which WebGPU
/// takes only in uniform control flow; and two loads lie between them, a
/// lane's own, so they are no one load every lane shares
/// (pxl-shader-build's `workgroupUniformLoad`). `ways_cs`'s and
/// `ways_tree_cs`'s one scan.
fn scan(sums: &mut Shared, t: u32, own: u32) -> u32 {
    let at = |v: u32| (v & (fill::SCAN_THREADS - 1)) as usize;
    // SAFETY: every index is masked under the array's length.
    unsafe { *sums.index_unchecked_mut(at(t)) = own };
    let mut d = 1;
    while d < fill::SCAN_THREADS {
        spirv_std::arch::workgroup_memory_barrier_with_group_sync();
        // SAFETY: masked, as above.
        let (before, sum) = unsafe { (*sums.index_unchecked(at(t.wrapping_sub(d))), *sums.index_unchecked(at(t))) };
        spirv_std::arch::workgroup_memory_barrier_with_group_sync();
        // SAFETY: masked, as above.
        unsafe { *sums.index_unchecked_mut(at(t)) = sum + if t >= d { before } else { 0 } };
        d *= 2;
    }
    // SAFETY: masked, as above.
    unsafe { *sums.index_unchecked(at(t)) }
}

/// Word `i` of the workgroup's `w` as every lane reads it alike: the
/// barriers about the one load, which pxl-shader-build reads back as
/// WGSL's `workgroupUniformLoad`, so a loop bound on it is uniform.
fn shared(w: &Shared, i: u32) -> u32 {
    spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    // SAFETY: masked under the array's length.
    let v = unsafe { *w.index_unchecked((i & (fill::SCAN_THREADS - 1)) as usize) };
    spirv_std::arch::workgroup_memory_barrier_with_group_sync();
    v
}

/// Word `i` of the workgroup's `w`, as the lane that wrote it reads it.
fn lane_word(w: &Shared, i: u32) -> u32 {
    // SAFETY: masked under the array's length.
    unsafe { *w.index_unchecked((i & (fill::SCAN_THREADS - 1)) as usize) }
}

/// Word `i` of the workgroup's `w` set to `v`, by lane 0 alone.
fn set_shared(w: &mut Shared, t: u32, i: u32, v: u32) {
    if t == 0 {
        // SAFETY: masked under the array's length.
        unsafe { *w.index_unchecked_mut((i & (fill::SCAN_THREADS - 1)) as usize) = v };
    }
}

/// Every lane's writes to storage seen by every lane of the workgroup
/// after it, WGSL's `storageBarrier`: a level's slots, written by the lanes
/// of the one before, read by others.
fn settle() {
    spirv_std::arch::control_barrier::<
        { spirv_std::memory::Scope::Workgroup as u32 },
        { spirv_std::memory::Scope::Workgroup as u32 },
        { spirv_std::memory::Semantics::UNIFORM_MEMORY.bits() | spirv_std::memory::Semantics::ACQUIRE_RELEASE.bits() },
    >();
}

/// The ways' pool's free stack (pxl-spec-core `ways::tree::POOL_BLOCKS`
/// words), which starts as WebGPU makes a buffer, zeroed: word `TAKEN`
/// how many blocks are taken, the `free` after it the free blocks' ids,
/// block 0 never among them, a square's root word 0 naming no tree as a
/// heap word 0 names no children. A slot's id is read and written by `id`
/// and `put_id` alone, a slot's word its id XOR its index, so a slot never
/// written holds its own index: every block free, with no start to write.
const TAKEN: u32 = 0;

/// The blocks free when `taken` are, block 0 never one.
fn free(taken: u32) -> u32 {
    tree::POOL_BLOCKS - 1 - taken
}

/// The id stack slot `i` holds.
fn id(stack: &[u32], i: u32) -> u32 {
    at(stack, i) ^ i
}

/// Block `block` into stack slot `i`.
fn put_id(stack: &mut [u32], i: u32, block: u32) {
    put(stack, i, block ^ i);
}

/// An atomic add of `v` on word `i` of `s`, its old value; none past `s`.
/// Bounded here, not by an index that panics: a panic's path is an early
/// return, which rust-gpu carries out of every loop about it as a flag a
/// lane's own, and no barrier after such a loop is in uniform control flow.
fn add(s: &mut [u32], i: u32, v: u32) -> u32 {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(s.get_unchecked_mut(i as usize), v) }
    } else {
        0
    }
}

/// Block `block` back on the stack.
fn push(stack: &mut [u32], block: u32) {
    // One fewer taken: an add of -1, as the count's word is unsigned.
    let k = free(add(stack, TAKEN, 1u32.wrapping_neg()));
    put_id(stack, 1 + k, block);
}

/// `n` blocks off the stack together for a square that has taken `taken`:
/// where their ids begin in it, and false where fewer are free or the
/// square would take more than its most (`ways::tree::fits`), none taken.
/// One exit, for the reason `add` gives.
fn pop(stack: &mut [u32], taken: u32, n: u32) -> (u32, bool) {
    // SAFETY: word `TAKEN`, the stack's first; the buffer is never empty
    // (pxl-render gpu/renderer.rs makes it `POOL_BLOCKS` words).
    let count = unsafe { stack.get_unchecked_mut(TAKEN as usize) };
    let mut gone = unsafe { spirv_std::arch::atomic_load::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(count) };
    let (mut done, mut ok) = (false, false);
    while !done {
        if !tree::fits(free(gone), taken, n) {
            done = true;
        } else {
            // SAFETY: as above.
            let was = unsafe {
                spirv_std::arch::atomic_compare_exchange::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }, { Semantics::NONE.bits() }>(count, gone + n, gone)
            };
            (done, ok) = (was == gone, was == gone);
            if !done {
                gone = was;
            }
        }
    }
    (if ok { 1 + free(gone) - n } else { 0 }, ok)
}

use tree::{FREED_AT, RECORDS_AT, RECORD_WORDS};

/// Block `first / 8`'s record, its id and its sixteen words, the next of
/// the scratch at `at`.
fn record(scratch: &mut [u32], at: u32, pool: &impl Pool, first: u32) {
    let r = add(scratch, at + RECORDS_AT, 1);
    let w = at + RECORDS_AT + 1 + r * RECORD_WORDS;
    put(scratch, w, first / 8);
    // Off the pool word by word, as `tree::lay` reads them, no array
    // between: a runtime index into one is a bounds check, whose panic's
    // path no loop about it may carry (`add`).
    let mut k = 0;
    while k < 8 {
        put(scratch, w + 1 + k, pool.heap(first + k));
        put(scratch, w + 9 + k, pool.data(first + k));
        k += 1;
    }
}

/// Block `block` the build gave back, into the scratch at `at`.
fn freed(scratch: &mut [u32], at: u32, block: u32) {
    let r = add(scratch, at + FREED_AT, 1);
    put(scratch, at + FREED_AT + 1 + r, block);
}

/// Each re-filled square's old ways' tree given back to the pool, a
/// workgroup a square, ahead of its build (`ways_tree_cs`): walked from its
/// root word, depth first, every block its own and its branches' pushed.
#[spirv(compute(threads(64, 1, 1)))]
pub fn ways_free_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] stack: &mut [u32],
) {
    let root = at(ground, square_at(at(fresh, window::handed_at(group.z) + window::FRESH_SQUARE)) + window::ROOT_AT);
    if lane.x != 0 || root == 0 {
        return;
    }
    let heap = |slot: u32| at(pool, slot);
    // Each level's block and the next of its eight to look under.
    let mut path = [[0u32; 2]; LEAF_CELL as usize + 1];
    let (mut d, mut first) = (0usize, root);
    path[0] = [root, 0];
    push(stack, root / 8);
    loop {
        let [b, k] = path[d];
        if k == 8 {
            if d == 0 {
                break;
            }
            d -= 1;
            continue;
        }
        path[d][1] = k + 1;
        let h = heap(b + k);
        if h != 0 {
            first = h & !overwrite::BRANCH;
            push(stack, first / 8);
            d += 1;
            path[d] = [first, 0];
        }
    }
    let _ = first;
}

/// Each handed square's ways as a tree of the placed tree's shape
/// (pxl-spec-core `ways::tree`), a workgroup a square, `group.z` its place
/// in `fresh`: from the world's zoom-0 block down a level at a time, each
/// candidate flagged where the square's levels say a way may lie under it
/// and the flagged given their children's blocks off the pool's stack
/// together, in the order the workgroup's scan ranks them; at the square's
/// cells each a leaf off its column's runs; then up, each branch folded,
/// one of eight equal leaves written as that leaf and its block given back
/// (`ways_freed_cs` pushes them), every block it holds kept as a record for
/// the read-back. Its root word last; none where the pool ran out, the
/// blocks it took given back. A level's numbers are what every lane reads
/// alike (`shared`), the loops' bounds.
#[spirv(compute(threads(64, 1, 1)))]
#[allow(clippy::too_many_arguments)]
pub fn ways_tree_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] ground: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] pool: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] stack: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] scratch: &mut [u32],
    #[spirv(workgroup)] sums: &mut Shared,
    #[spirv(workgroup)] levels: &mut Shared,
) {
    let (k, t, lanes) = (group.z, lane.x, fill::SCAN_THREADS);
    let of = fill::of(fresh, k);
    let base = square_at(at(fresh, window::handed_at(k) + window::FRESH_SQUARE));
    let mine = fill::scratch_at(k);
    let mut pool = tree::Lane(pool);
    // A level's candidate `c`: slot `c % 8` of the block whose id is the
    // level's `c / 8`-th off the stack, from where its ids begin.
    let slot = |stack: &[u32], from: u32, c: u32| 8 * id(stack, from + c / 8) + c % 8;
    // levels[2j], levels[2j + 1]: level j's first id on the stack and its
    // candidates; the last three words, the blocks the square has taken, its
    // cells' zoom and whether the pool held every level so far.
    let (taken, cells, held) = (lanes - 3, lanes - 2, lanes - 1);
    if t == 0 {
        put(scratch, mine + FREED_AT, 0);
        put(scratch, mine + RECORDS_AT, 0);
        let (from, ok) = pop(stack, 0, 1);
        set_shared(levels, t, taken, 1);
        if ok {
            tree::root(&mut pool, of, 8 * id(stack, from));
        }
        set_shared(levels, t, 0, from);
        set_shared(levels, t, 1, if ok { 8 } else { 0 });
        set_shared(levels, t, cells, of.cell);
        set_shared(levels, t, held, ok as u32);
    }
    let (cell, first) = (shared(levels, cells), shared(levels, 0));
    let (mut j, mut zoom) = (0, 0);
    let mut ok = shared(levels, held) != 0;
    while ok && zoom < cell {
        settle();
        let (from, n) = (shared(levels, 2 * j), shared(levels, 2 * j + 1));
        let square = fill::Fresh { ground, base };
        // The flagged, counted, then given their blocks together.
        let (mut flagged, mut c) = (0, 0);
        while c < n {
            // Asked of every lane, its own past the level or not, then
            // chosen: a short circuit would put the scan after it in a
            // branch of the lane's own, where no barrier may be.
            let i = c + t;
            let flag = tree::flag(&pool, &square, 0, of, zoom, slot(stack, from, i)) & (i < n);
            scan(sums, t, flag as u32);
            flagged += shared(sums, lanes - 1);
            c += lanes;
        }
        if t == 0 {
            let had = lane_word(levels, taken);
            let (next, fits) = pop(stack, had, flagged);
            set_shared(levels, t, taken, had + flagged);
            set_shared(levels, t, 2 * j + 2, next);
            set_shared(levels, t, 2 * j + 3, 8 * flagged);
            set_shared(levels, t, held, fits as u32);
        }
        ok = shared(levels, held) != 0;
        let next = shared(levels, 2 * j + 2);
        let (mut before, mut c) = (0, 0);
        while ok && c < n {
            let i = c + t;
            let s = slot(stack, from, i);
            let flag = tree::flag(&pool, &square, 0, of, zoom, s) & (i < n);
            let rank = before + scan(sums, t, flag as u32) - flag as u32;
            if flag {
                tree::branch(&mut pool, of, zoom, s, 8 * id(stack, next + rank));
            } else if i < n {
                tree::empty(&mut pool, s);
            }
            before += shared(sums, lanes - 1);
            c += lanes;
        }
        (j, zoom) = (j + 1, zoom + 1);
    }
    // The pool ran out: the blocks each level before took, given back.
    if !ok && t == 0 {
        let mut level = 0;
        while level < j {
            let (from, n) = (lane_word(levels, 2 * level), lane_word(levels, 2 * level + 1));
            let mut b = 0;
            while b < n / 8 {
                freed(scratch, mine, id(stack, from + b));
                b += 1;
            }
            level += 1;
        }
    }
    settle();
    let (from, n) = (shared(levels, 2 * j), shared(levels, 2 * j + 1));
    // Every loop a lane's share steps the same chunks, its bound what every
    // lane reads alike, so the barriers after it are in uniform control
    // flow, which WebGPU takes only there.
    let mut c = 0;
    while ok && c < n {
        let runs = fill::Scratch { words: scratch, at: mine };
        if c + t < n {
            tree::leaf(&mut pool, &runs, of, slot(stack, from, c + t));
        }
        c += lanes;
    }
    // Up: each level's branches folded over the level under them.
    while ok && j > 0 {
        settle();
        (j, zoom) = (j - 1, zoom - 1);
        let (from, n) = (shared(levels, 2 * j), shared(levels, 2 * j + 1));
        let mut c = 0;
        while c < n {
            let s = slot(stack, from, c + t);
            let under = tree::under(&pool, s);
            let first = if c + t < n { under } else { tree::NONE };
            if first != tree::NONE {
                let gave = tree::fold(&mut pool, zoom, s);
                if gave != tree::NONE {
                    freed(scratch, mine, gave / 8);
                } else {
                    record(scratch, mine, &pool, first);
                }
            }
            c += lanes;
        }
    }
    settle();
    if t == 0 {
        let root = if ok { 8 * id(stack, first) } else { 0 };
        if ok {
            record(scratch, mine, &pool, root);
        }
        put(ground, base + window::ROOT_AT, root);
    }
}

/// The blocks each handed square's build gave back (`ways_tree_cs`), on the
/// stack again: after the build, so no push lands under another square's
/// ids while it reads them.
#[spirv(compute(threads(64, 1, 1)))]
pub fn ways_freed_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] scratch: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] stack: &mut [u32],
) {
    let mine = fill::scratch_at(group.z);
    let n = at(scratch, mine + FREED_AT);
    let mut r = lane.x;
    while r < n {
        push(stack, at(scratch, mine + FREED_AT + 1 + r));
        r += fill::SCAN_THREADS;
    }
}

/// The eye's slots whose decor grows again this frame, a thread a slot,
/// listed with each kind shown at the slot's window zoom for `decor_cs`
/// (pxl-spec-core `scatter::GROWS_AT`): bit `zoom − FIRST_CELL` of its
/// `shown` word set, the kinds the catalog's rows as handed (`rows`);
/// its count the dispatch's: every one where `marks` asks it, decor having
/// been off, else one whose
/// square its records were not grown for (`scatter::grown_for`), beside a
/// square filled this frame (`fresh`), or met by a box (`scatter::meets`):
/// an edit, or a preview's cut as it was or is. A listed slot's head is
/// laid anew, its count none.
/// Whether kind `j` is shown at the window zoom `cell`: bit `cell −
/// FIRST_CELL` of its `shown` word, at a leaf's cells or coarser.
fn shown_at(shown: &[u32], j: u32, cell: u32) -> bool {
    cell <= LEAF_CELL && at(shown, j) >> (cell - FIRST_CELL) & 1 != 0
}

#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_mark_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] marks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] fresh: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] records: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] grows: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] shown: &[u32],
) {
    let s = id.x;
    if s >= scatter::EYE_SLOTS {
        return;
    }
    let held = at(window, s);
    if held.z == 0 {
        return;
    }
    let (cell, q, h) = (window::eye_cell(s / SLOTS), [held.x, held.y], scatter::head_at(s));
    let head = [at(records, h), at(records, h + 1), at(records, h + 2), at(records, h + 3)];
    let mut marked = at(marks, 0) != 0 || !scatter::grown_for(head, q);
    let mut k = 0;
    while k < at(marks, 1) {
        let f = at(fresh, window::handed_at(k) + window::FRESH_SQUARE);
        if f < scatter::EYE_SLOTS && f / SLOTS == s / SLOTS {
            let other = at(window, f);
            marked = marked || scatter::beside(cell, q, [other.x, other.y]);
        }
        k += 1;
    }
    k = 0;
    while k < at(marks, 2) {
        let b = scatter::MARKS_AT + 4 * k;
        let edit = [at(marks, b) as i32, at(marks, b + 1) as i32, at(marks, b + 2) as i32, at(marks, b + 3) as i32];
        marked = marked || scatter::meets(cell, q, edit);
        k += 1;
    }
    if !marked {
        return;
    }
    put(records, h, 0);
    put(records, h + 1, q[0] as u32);
    put(records, h + 2, q[1] as u32);
    put(records, h + 3, 1);
    let kinds = rows.len() as u32;
    let mut j = 0;
    while j < kinds {
        if shown_at(shown, j, cell) {
            // SAFETY: an atomic add on the list's count, the dispatch's z.
            let n = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut grows[2], 1) };
            put(grows, scatter::GROWS_AT + n, s | j << 16);
        }
        j += 1;
    }
}

/// Decor the ground grows in a slot `decor_mark_cs` listed, one candidate a
/// thread: `x` its index in its block, `y` the block among those meeting the
/// slot's square (pxl-spec-core `scatter::block_of`), `z` the slot and kind
/// listed. Each one that stands, stands on its cell and grows there
/// (`scatter::stands`, `grown`) is added to the slot's records
/// (`scatter::record_at`), naming the slot's window zoom, the one it is
/// drawn at where the rays ask it (`decor_count_cs`); only a kind shown at
/// that zoom is listed (`decor_mark_cs`). What it grows is the windows'
/// squares', the ways over them and what is placed alone, never the eye's.
#[spirv(compute(threads(8, 8, 1)))]
pub fn decor_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] out: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] grows: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] pool: &[u32],
) {
    let listed = at(grows, scatter::GROWS_AT + id.z);
    let (square, kind) = (listed & 0xffff, listed >> 16);
    let cell = window::eye_cell(square / SLOTS);
    if id.x >= COUNTS as u32 || id.y >= SQUARE_BLOCKS {
        return;
    }
    let held = at(window, square);
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
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        },
        rows,
    };
    let decor = &rows[kind as usize];
    let zoom = GROUND.read_zoom(cell);
    let (c, here) = scatter::candidate_of(decor, zoom, region, id.y, id.x);
    if !here || !scatter::stands(decor, zoom, &c) {
        return;
    }
    let (g, here) = scatter::grown(&mut world, decor, cell, &c, true);
    if !here {
        return;
    }
    // SAFETY: an atomic add on the slot's head, its count.
    let n = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut out[scatter::head_at(square) as usize], 1) };
    let i = scatter::record_at(square * SQUARE_GROWN + n).0 as usize;
    if n < SQUARE_GROWN && i + 4 < out.len() {
        out[i] = scatter::record_word(kind, g.variant, g.turn, cell);
        out[i + 1] = g.at[0] as u32;
        out[i + 2] = g.at[1] as u32;
        out[i + 3] = g.at[2] as u32;
        out[i + 4] = g.sink;
    }
}

/// Lanes a decor pass's workgroup has (`decor_count_cs`, `decor_place_cs`).
const RECORD_LANES: u32 = 64;

/// The records workgroup `g` of a decor pass reads, as record numbers
/// (pxl-spec-core `scatter::record_at`): the first and how many. A workgroup
/// a slot, its records to its head's count where the windows still hold the
/// square they were grown for (`scatter::grown_for`), asked once for them
/// all, none where they do not; past the slots, a workgroup a
/// `RECORD_LANES` of placed decor's. Its lanes take them `RECORD_LANES`
/// apart, so a pass reads the records the heads hold, not the room.
fn records(grown: &[u32], window: &[IVec4], g: u32) -> (u32, u32) {
    if g < scatter::EYE_SLOTS {
        let (h, q) = (scatter::head_at(g), at(window, g));
        let head = [at(grown, h), at(grown, h + 1), at(grown, h + 2), at(grown, h + 3)];
        let held = q.z != 0 && scatter::grown_for(head, [q.x, q.y]);
        let count = if head[0] < SQUARE_GROWN { head[0] } else { SQUARE_GROWN };
        (g * SQUARE_GROWN, if held { count } else { 0 })
    } else {
        let first = (g - scatter::EYE_SLOTS) * RECORD_LANES;
        let count = at(grown, scatter::PLACED_HEAD);
        let left = if count > first { count - first } else { 0 };
        (scatter::SLOT_RECORDS + first, if left < RECORD_LANES { left } else { RECORD_LANES })
    }
}

/// Each record for the eye this frame, a workgroup a slot's or a run of
/// placed decor's (`records`), its region written into its first word and
/// counted: nothing where the rays ask another zoom at its
/// distance than the one it was grown at (`scatter::drawn_at`), or where it
/// is the decor left out; else sized for the screen (pxl-spec-core
/// `scatter::drawn`), a model where its group's extent spans `MODEL_PX` at
/// its distance, a sprite down to `SPRITE_PX`, else nothing.
#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_count_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] grown: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    let (first, count) = records(grown, window, group.x);
    let mut k = lane.x;
    while k < count {
        count_one(eye, params, rows, grown, regions, scatter::record_at(first + k).0 as usize);
        k += RECORD_LANES;
    }
}

/// `decor_count_cs` for the record at word `i`.
fn count_one(eye: &Eye, params: &[u32], rows: &[Decor], grown: &mut [u32], regions: &mut [u32], i: usize) {
    if i + 3 >= grown.len() {
        return;
    }
    let w = grown[i];
    let (kind, variant, _, cell) = scatter::record_of(w);
    let corner = [grown[i + 1] as i32, grown[i + 2] as i32, grown[i + 3] as i32];
    let (origin, detail) = eye_origin(eye);
    // Its distance once, for the zoom it is drawn at and its size.
    let d2 = scatter::distance2(origin, corner);
    let here = cell == scatter::PLACED_AT || scatter::drawn_at(detail, d2, cell);
    let group = scatter::first_group(rows, kind) + variant;
    let hide = drawn::HIDE as usize;
    let hidden = params[hide + 3] != 0 && corner[0] == params[hide] as i32 && corner[1] == params[hide + 1] as i32 && corner[2] == params[hide + 2] as i32;
    let d = spirv_std::num_traits::Float::sqrt(d2);
    let d = if d > 1e-3 { d } else { 1e-3 };
    let px = f32::from_bits(params[(drawn::EXTENTS + group) as usize]) / (d * f32::from_bits(params[drawn::UPP as usize]));
    let tier = drawn::tier(px, f32::from_bits(params[drawn::MODEL_PX as usize]), f32::from_bits(params[drawn::SPRITE_PX as usize]));
    let region = if hidden || !here || tier == drawn::GONE { drawn::NONE } else { 2 * group + tier };
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

/// Each record its region drew a slot of, read as `decor_count_cs` reads
/// them (`records`): its box's base middle about the
/// anchor, its heading and scale, in the lanes the decor and impostor
/// shaders read, and the slot its own in the index list.
#[spirv(compute(threads(64, 1, 1)))]
pub fn decor_place_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] params: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] rows: &[Decor],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] grown: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 3)] regions: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] block: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] window: &[IVec4],
) {
    let (first, count) = records(grown, window, group.x);
    let mut k = lane.x;
    while k < count {
        place_one(params, rows, grown, regions, block, scatter::record_at(first + k).0 as usize);
        k += RECORD_LANES;
    }
}

/// `decor_place_cs` for the record at word `i`.
fn place_one(params: &[u32], rows: &[Decor], grown: &[u32], regions: &mut [u32], block: &mut [u32], i: usize) {
    if i + 4 >= grown.len() {
        return;
    }
    let w = grown[i];
    let region = w >> drawn::REGION_SHIFT;
    if region == drawn::NONE {
        return;
    }
    // SAFETY: an atomic add on the region's next slot, a word of the buffer.
    let slot = unsafe { spirv_std::arch::atomic_i_add::<u32, { Scope::QueueFamily as u32 }, { Semantics::NONE.bits() }>(&mut regions[(drawn::REGIONS + region) as usize], 1) };
    let (kind, variant, turn, _) = scatter::record_of(w);
    let [across, deep, _] = rows[kind as usize].cells(variant, turn);
    let room = block.len() as u32 / (drawn::LANES + 1);
    if slot >= room {
        return;
    }
    let anchor = drawn::ANCHOR as usize;
    // X about the anchor the short way round: leaf cells wrap at 2^24.
    let x = (((grown[i + 1] as i32).wrapping_sub(params[anchor] as i32)) << 8 >> 8) as f32 + across as f32 / 2.0;
    let y = (grown[i + 2] as i32).wrapping_sub(params[anchor + 1] as i32) as f32 + deep as f32 / 2.0;
    // Its base as drawn: under its box's corner by its footing's kept top.
    let sink = grown[i + 4] as f32 / (1 << pxl_spec_core::FRAC_W) as f32;
    let z = (grown[i + 3] as i32).wrapping_sub(params[anchor + 2] as i32) as f32 - sink;
    let (room, slot) = (room as usize, slot as usize);
    block[slot] = x.to_bits();
    block[room + slot] = y.to_bits();
    block[2 * room + slot] = z.to_bits();
    block[3 * room + slot] = ((turn % 4) as f32 * core::f32::consts::FRAC_PI_2).to_bits();
    block[4 * room + slot] = 1.0f32.to_bits();
    block[drawn::LANES as usize * room + slot] = slot as u32;
}
