// pxl-shader/world.rs

//! The world as the GPU holds it, asked through pxl-spec-core's one query
//! (`world::node`) by every entry that walks or grows over it.

use pxl_spec_core::answer::{self, In};
use pxl_spec_core::area::Area;
use pxl_spec_core::cast::{self, Ends, Met, Query};
use pxl_spec_core::fill::{self, column_of, level_of};
use pxl_spec_core::overwrite::{Cut, Lanes};
use pxl_spec_core::window::{self, Found, Squares, FIRST_CELL, SLOTS, SQUARE_CELLS, SQUARE_WORDS};
use pxl_spec_core::world::{self, At};
use pxl_spec_core::{at, face, ground, support, way};
use spirv_std::glam::IVec4;

use crate::eye::Eye;

/// A preview's entry as pxl-spec-core `overwrite` takes it.
fn cut(e: IVec4) -> Cut {
    Cut {
        zoom: e.w as u32,
        at: [e.x, e.y, e.z],
    }
}

/// Square `square`'s first word in the ground's buffer (pxl-spec-core
/// `window::SQUARE_WORDS`).
pub(crate) fn square_at(square: u32) -> u32 {
    square * SQUARE_WORDS
}

/// The world as the GPU holds it (pxl-kernel domain/scene/cast.rs fills it): the
/// squares the eye's windows hold, their columns, levels and runs, the
/// service area, and what is placed with a preview's cuts; the caves are a
/// hash, held nowhere. Decor is not here (pxl-spec-core `cast::Query`).
/// The squares' cells' four looks are `L`'s: their own buffer for the
/// cast's walks, every observer's alike, none where a pass reads no cell's
/// look (`Unlooked`).
#[derive(Clone, Copy)]
pub struct World<'a, L> {
    /// The windows' table (pxl-spec-core `window::Slot`): every window's
    /// slots, a row each, the square it wants, the berth holding it plus one
    /// while present (`Slot::berth`) and the fill it is on its way from,
    /// the eye's windows first, from the top's; then each window's first
    /// square (`window::ORIGIN_AT`) and the eye's windows' folds
    /// (`window::JOINED_AT`).
    pub(crate) window: &'a [IVec4],
    /// Per berth (`window::SQUARES`), its square's columns (`height_cs`,
    /// `way_cs`), their levels (`bound_cs`), their runs (`way_cs`) and its
    /// cells as laid (`height_cs`) (`window::SQUARE_WORDS`), whichever
    /// window filled it.
    pub(crate) ground: &'a [u32],
    /// Per berth, in the same order, its cells' four looks (`Looks`).
    pub(crate) looks: L,
    /// The service area's index (pxl-spec-core `area::Area`), its keys four a
    /// row.
    pub(crate) shards: &'a [IVec4],
    /// The overwrites' tree (pxl-kernel domain/layer/object/store.rs): its heap
    /// lane, then its data lane from half the buffer.
    pub(crate) overwrites: &'a [u32],
    /// The eye, whose preview cuts are read where something placed or cut
    /// is asked about, not held through the descent.
    pub(crate) eye: &'a Eye,
    /// The ways' pool (pxl-spec-core `way::tree`), every square's tree,
    /// which every pass that asks the world's query reads.
    pub(crate) pool: way::tree::Read<'a>,
}

/// Where the squares' cells' four looks are read: a word a cell
/// (`ground::looks`), `window::SQUARE_CELLS` a square in the squares' order,
/// as the fill wrote them off the handed record (`height_cs`).
pub trait Looks: Copy {
    fn look(&self, i: u32) -> u32;
}

impl Looks for &[u32] {
    fn look(&self, i: u32) -> u32 {
        at(self, i)
    }
}

/// No looks bound: a pass that reads no cell's look, every cell's four the
/// word it holds, none (`ground::word`'s zero). A word, not nothing:
/// rust-gpu offsets no pointer to a field of no size.
#[derive(Clone, Copy)]
pub struct Unlooked(pub(crate) u32);

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
    /// Its cells' nines four a row, by key (pxl-kernel domain/scene/cast.rs
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

/// The eye's windows, as the GPU holds them: a square in the berth its
/// slot's row names, whichever window filled it.
impl<L: Looks> Squares for World<'_, L> {
    /// The berth square `q` is held in, while the eye's window holds it
    /// present.
    fn present(&self, cell: u32, q: [i32; 2]) -> (u32, bool) {
        let row = self.row((cell - FIRST_CELL) * SLOTS + window::slot(q));
        (row.berth(), row.holds(q))
    }
    fn square(&self, cell: u32, slot: u32) -> ([i32; 2], bool) {
        self.row((cell - FIRST_CELL) * SLOTS + slot).held()
    }
    /// Off the windows' rows past their table (`joined_cs`).
    fn joined(&self, cell: u32, e: u32) -> [u32; window::JOINED_WORDS as usize] {
        let r = at(
            self.window,
            window::JOINED_AT + (cell - FIRST_CELL) * window::JOINED_ENTRIES + e,
        );
        [r.x as u32, r.y as u32, r.z as u32, r.w as u32]
    }
    /// The window's row past every window's slots (`window::ORIGIN_AT`).
    fn origin(&self, cell: u32) -> ([i32; 2], bool) {
        self.row(window::ORIGIN_AT + (cell - FIRST_CELL)).held()
    }
    fn look(&self, _: u32, berth: u32, i: u32) -> u32 {
        self.looks.look(berth * SQUARE_CELLS + i)
    }
    fn column(&self, _: u32, berth: u32, i: u32) -> u32 {
        column_of(self.ground, square_at(berth), i)
    }
    fn level(&self, _: u32, berth: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        level_of(self.ground, square_at(berth), j, at)
    }
    fn laid(&self, _: u32, berth: u32, i: u32) -> u32 {
        fill::laid_of(self.ground, square_at(berth), i)
    }
    fn root(&self, _: u32, berth: u32) -> u32 {
        fill::root_of(self.ground, square_at(berth))
    }
}

/// The ground as the GPU holds it, for pxl-spec-core's one answer for a node.
impl<L: Looks> ground::Held for World<'_, L> {
    /// The bound over the node of the window `cell` reads its cells from,
    /// the finest holding them present (`window::bound_over`), as the CPU reads
    /// it (pxl-spec-core `Held::relief`).
    fn relief(
        &mut self,
        place: &mut Found,
        cells: [u32; 2],
        (zoom, c): (u32, [i32; 3]),
    ) -> (ground::Bounds, bool) {
        let (r, _, here) = window::bound_over(self, place, cells, (zoom, [c[0], c[1]]));
        (r, here)
    }

    /// The cell's column off the windows, the area's edge laid on it
    /// (pxl-spec-core `window::ground`), as the CPU reads it (pxl-kernel
    /// domain/query/node.rs `Reading::cell`).
    fn cell(&mut self, place: Found, zoom: u32, c: [i32; 3]) -> ground::Cell {
        window::ground(self, place, zoom, [c[0], c[1]]).0
    }
}

/// What a node is on the GPU: its value, one of the catalog's whichever
/// source holds it, and the four looks the ground's asset is drawn by
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
    fn node(
        &mut self,
        at: &mut At,
        zoom: u32,
        c: [i32; 3],
        asked: [u32; 2],
        _t0: f32,
    ) -> In<[u32; 2]> {
        let (w, cuts) = (*self, [cut(self.eye.carve), cut(self.eye.hide)]);
        world::node(self, &w, &w, &w.pool, at, (zoom, c), asked, cuts)
    }
}

impl<L: Looks> World<'_, L> {
    /// What lies about the face a ray stopped on (pxl-spec-core
    /// `face::word`): each cell beside the stop's at its zoom, hiding its
    /// cube whole (`answer::whole`), asked off the line down to the stop's
    /// node (`support::asset`), each from the deepest node it shares with
    /// it, never again from the top: `line`'s, the line its walk left where
    /// it kept one (observed.rs `Lined`), else one walked down to it (`support::line`); a
    /// box's own, a cell kept short of its cube (`face::BOX`); nothing where
    /// the ray stopped on none.
    pub(crate) fn faced<A: Copy>(
        &mut self,
        origin: [i32; 3],
        ray: &cast::Ray,
        met: &Met<A>,
        line: impl FnOnce(&mut Self, u32, [i32; 3]) -> support::Line<At>,
    ) -> u32 {
        if met.ends != Ends::Stop {
            return 0;
        }
        if answer::kept(met.stop.asset[0]) < answer::EIGHTHS {
            return face::BOX;
        }
        let (zoom, c) = (met.stop.node.zoom, met.stop.node.index(origin));
        let line = line(self, zoom, c);
        face::word(face_of(ray, met.stop.axis), |d| {
            answer::whole(support::asset(self, &line, zoom, face::beside(zoom, c, d)))
        })
    }
}

/// The face a ray enters a cell through across `axis` (pxl-spec-core
/// `ray`): 2·axis, plus one where its outward normal points down the axis,
/// against the ray.
pub(crate) fn face_of(ray: &cast::Ray, axis: u32) -> u32 {
    2 * axis + (pxl_spec_core::axis(ray.direction, axis) > 0.0) as u32
}
