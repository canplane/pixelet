// pxl-shader/world.rs

//! The world as the GPU holds it, asked through pxl-spec-core's one query
//! (`world::node`) by every entry that walks or generates over it.

use pxl_spec_core::answer::{self, In};
use pxl_spec_core::area::Area;
use pxl_spec_core::cast::{self, End, Query};
use pxl_spec_core::fill::{self, column_of, level_of};
use pxl_spec_core::overwrite::{Cut, Columns};
use pxl_spec_core::clipmap::{
    self, Bricks, Lookup, Presence, BRICK_CELLS, BRICK_WORDS, ENTRIES, FIRST_CELL,
};
use pxl_spec_core::world::{self, Inheritance};
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

/// Brick `brick`'s first word in the ground's buffer (pxl-spec-core
/// `clipmap::BRICK_WORDS`).
pub(crate) fn brick_at(brick: u32) -> u32 {
    brick * BRICK_WORDS
}

/// The world as the GPU holds it (pxl-kernel domain/scene/cast.rs fills it): the
/// bricks the eye's clip levels hold, their columns, levels and runs, the
/// service area, and what is placed with a preview's cuts; the caves are a
/// hash, held nowhere. Decor is not here (pxl-spec-core `cast::Query`).
/// The bricks' cells' four palette indices are `L`'s: their own buffer for the
/// cast's walks, every observer's alike, none where a pass reads no cell's
/// palette index (`NoPalette`).
#[derive(Clone, Copy)]
pub struct World<'a, L> {
    /// The clip levels' table (pxl-spec-core `clipmap::Entry`): every clip
    /// level's entries, a row each, the brick it wants, the slot holding it
    /// plus one while present (`Entry::slot`) and the fill it is on its way
    /// from, the eye's clip levels first, from the top's; then each clip
    /// level's first brick (`clipmap::ORIGIN_AT`) and the eye's clip levels'
    /// folds (`clipmap::JOINED_AT`).
    pub(crate) clipmap: &'a [IVec4],
    /// Per slot (`clipmap::SLOTS`), its brick's columns (`height_cs`,
    /// `way_cs`), their levels (`bound_cs`), their runs (`way_cs`) and its
    /// cells as laid (`height_cs`) (`clipmap::BRICK_WORDS`), whichever
    /// clip level filled it.
    pub(crate) ground: &'a [u32],
    /// Per slot, in the same order, its cells' four palette indices (`PaletteWords`).
    pub(crate) palette_indices: L,
    /// The service area's index (pxl-spec-core `area::Area`), its keys four a
    /// row.
    pub(crate) shards: &'a [IVec4],
    /// The overwrites' tree (pxl-kernel domain/layer/object/store.rs): its child
    /// pointer lane, then its data lane from half the buffer.
    pub(crate) overwrites: &'a [u32],
    /// The eye, whose preview cuts are read where something placed or cut
    /// is asked about, not held through the descent.
    pub(crate) eye: &'a Eye,
    /// The ways' pool (pxl-spec-core `way::tree`), every brick's tree,
    /// which every pass that asks the world's query reads.
    pub(crate) pool: way::tree::Read<'a>,
}

/// Where the bricks' cells' four palette indices are read: a word a cell
/// (`ground::palette_indices`), `clipmap::BRICK_CELLS` a brick in the bricks' order,
/// as the fill wrote them off the handed record (`height_cs`).
pub trait PaletteWords: Copy {
    fn palette_word(&self, i: u32) -> u32;
}

impl PaletteWords for &[u32] {
    fn palette_word(&self, i: u32) -> u32 {
        at(self, i)
    }
}

/// No palette indices bound: a pass that reads no cell's palette index, every cell's four the
/// word it holds, none (`ground::word`'s zero). A word, not nothing:
/// rust-gpu offsets no pointer to a field of no size.
#[derive(Clone, Copy)]
pub struct NoPalette(pub(crate) u32);

impl PaletteWords for NoPalette {
    fn palette_word(&self, _: u32) -> u32 {
        self.0
    }
}

impl<L: PaletteWords> Columns for World<'_, L> {
    fn first_child(&self, slot: u32) -> u32 {
        at(self.overwrites, slot)
    }
    fn data(&self, slot: u32) -> u32 {
        at(self.overwrites, self.overwrites.len() as u32 / 2 + slot)
    }
}

impl<L: PaletteWords> Area for World<'_, L> {
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

/// The clip levels' table alone (`World::clipmap`), which bricks they hold
/// present: what a pass binding no brick asks of them (`clipmap::step`).
#[derive(Clone, Copy)]
pub struct Clipmap<'a>(pub(crate) &'a [IVec4]);

impl Clipmap<'_> {
    /// Row `r` of the clip levels' table (`clipmap::Entry`).
    fn row(&self, r: u32) -> clipmap::Entry {
        let h = at(self.0, r);
        clipmap::Entry::of([h.x, h.y, h.z, h.w])
    }
}

impl Presence for Clipmap<'_> {
    /// The slot brick `q` is held in, while the eye's clip level holds it
    /// present.
    fn present(&self, cell: u32, q: [i32; 2]) -> (u32, bool) {
        let row = self.row((cell - FIRST_CELL) * ENTRIES + clipmap::entry(q));
        (row.slot(), row.holds(q))
    }
}

impl<L> World<'_, L> {
    /// Row `r` of the clip levels' table (`Clipmap`).
    fn row(&self, r: u32) -> clipmap::Entry {
        Clipmap(self.clipmap).row(r)
    }
}

impl<L: PaletteWords> Presence for World<'_, L> {
    fn present(&self, cell: u32, q: [i32; 2]) -> (u32, bool) {
        Clipmap(self.clipmap).present(cell, q)
    }
}

/// The eye's clip levels, as the GPU holds them: a brick in the slot its
/// entry's row names, whichever clip level filled it.
impl<L: PaletteWords> Bricks for World<'_, L> {
    fn brick(&self, cell: u32, entry: u32) -> ([i32; 2], bool) {
        self.row((cell - FIRST_CELL) * ENTRIES + entry).brick()
    }
    /// Off the clip levels' rows past their table (`fold_cs`).
    fn joined(&self, cell: u32, e: u32) -> [u32; clipmap::JOINED_WORDS as usize] {
        let r = at(
            self.clipmap,
            clipmap::JOINED_AT + (cell - FIRST_CELL) * clipmap::JOINED_ENTRIES + e,
        );
        [r.x as u32, r.y as u32, r.z as u32, r.w as u32]
    }
    /// The clip level's row past every clip level's entries
    /// (`clipmap::ORIGIN_AT`).
    fn origin(&self, cell: u32) -> ([i32; 2], bool) {
        self.row(clipmap::ORIGIN_AT + (cell - FIRST_CELL)).brick()
    }
    fn palette_word(&self, _: u32, slot: u32, i: u32) -> u32 {
        self.palette_indices.palette_word(slot * BRICK_CELLS + i)
    }
    fn column(&self, _: u32, slot: u32, i: u32) -> u32 {
        column_of(self.ground, brick_at(slot), i)
    }
    fn level(&self, _: u32, slot: u32, j: u32, at: [u32; 2]) -> [i32; 2] {
        level_of(self.ground, brick_at(slot), j, at)
    }
    fn laid(&self, _: u32, slot: u32, i: u32) -> u32 {
        fill::laid_of(self.ground, brick_at(slot), i)
    }
    fn root(&self, _: u32, slot: u32) -> u32 {
        fill::root_of(self.ground, brick_at(slot))
    }
}

/// The ground as the GPU holds it, for pxl-spec-core's one answer for a node.
impl<L: PaletteWords> ground::Resident for World<'_, L> {
    /// The bound over the node of the clip level `cell` reads its cells from,
    /// the finest holding them present (`clipmap::bound_over`), as the CPU reads
    /// it (pxl-spec-core `ground::Resident::relief`).
    fn relief(
        &mut self,
        place: &mut Lookup,
        cells: [u32; 2],
        (zoom, c): (u32, [i32; 3]),
    ) -> (ground::Bounds, bool) {
        let (r, _, here) = clipmap::bound_over(self, place, cells, (zoom, [c[0], c[1]]));
        (r, here)
    }

    /// The cell's column off the clip levels, the area's edge laid on it
    /// (pxl-spec-core `clipmap::ground`), as the CPU reads it (pxl-kernel
    /// domain/query/node.rs `Reading::cell`).
    fn cell(&mut self, place: Lookup, zoom: u32, c: [i32; 3]) -> ground::Cell {
        clipmap::ground(self, place, zoom, [c[0], c[1]]).0
    }
}

/// What a node is on the GPU: its value, one of the catalog's whichever
/// source holds it, and the four palette indices the ground's asset is drawn by
/// (`world::node`'s second word), what the screen paints a hit with.
impl<L: PaletteWords> Query for World<'_, L> {
    type At = Inheritance;

    fn root(&mut self, node: [i32; 3]) -> Inheritance {
        Inheritance::of(self, (0, node))
    }

    fn child(&mut self, at: &Inheritance, zoom: u32, node: [i32; 3]) -> Inheritance {
        at.child(self, &self.pool, (zoom, node))
    }

    /// What is in the node `c` at `zoom`: the world's one query
    /// (pxl-spec-core `world::node`).
    fn node(
        &mut self,
        at: &mut Inheritance,
        zoom: u32,
        c: [i32; 3],
        zooms: [u32; 2],
        _t0: f32,
    ) -> In<[u32; 2]> {
        let (w, cuts) = (*self, [cut(self.eye.carve), cut(self.eye.hide)]);
        world::node(self, &w, &w, &w.pool, at, (zoom, c), zooms, cuts)
    }
}

impl<L: PaletteWords> World<'_, L> {
    /// What lies about face `face` of the cell `c` of `zoom` a ray stopped
    /// on (pxl-spec-core `face::word`): each cell beside it at its zoom,
    /// hiding its cube whole (`answer::whole`), asked off `line`, the line
    /// down to it read in place (`support::Ancestors`), each from the deepest
    /// node it shares with it, never again from the top (`support::asset`).
    /// Asked once a hit (raster.rs `face_cs`); a hit that needs none is
    /// told so first (`unasked_face`).
    pub(crate) fn ask_face(
        &mut self,
        face: u32,
        zoom: u32,
        c: [i32; 3],
        line: &impl support::Ancestors<Inheritance>,
    ) -> u32 {
        face::word(face, |d| {
            answer::whole(support::asset(self, line, zoom, face::beside(zoom, c, d)))
        })
    }
}

/// What lies about the face a hit that ended `ends` at the asset `asset`
/// met, where it needs no asking (`World::ask_face`): nothing where it stopped
/// on none, a box's own where the cell keeps short of its cube
/// (`face::BOX`); and whether it is asked instead.
pub(crate) fn unasked_face(ends: End, asset: u32) -> (bool, u32) {
    if ends != End::Hit {
        (false, 0)
    } else if answer::fill_eighths(asset) < answer::EIGHTHS {
        (false, face::BOX)
    } else {
        (true, 0)
    }
}

/// The face a ray enters a cell through across `axis` (pxl-spec-core
/// `ray`): 2·axis, plus one where its outward normal points down the axis,
/// against the ray.
pub(crate) fn face_of(ray: &cast::Ray, axis: u32) -> u32 {
    2 * axis + (pxl_spec_core::axis(ray.dir, axis) > 0.0) as u32
}
