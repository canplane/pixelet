// pxl-kernel/world/chunk/build/shape.rs

//! A chunk's runs: the zoom they are written at, the cells they fold and how wide a column of them is.

use pxl_spec::space::camera::grid::CHUNK_CELLS;
use pxl_spec::space::layer::lattice::GROUND;

use pxl_render::mesh::run::APRON;

use crate::layer::object::store::Store;
use crate::layer::Query;

use crate::world::chunk::Chunk as PyramidChunk;
use pxl_spec::space::coord::zoom::{count, units, Zoom};
use pxl_spec::space::hierarchy::tree::first_descendant_of;

/// The zoom a chunk's runs are written at: the records its cells fold
/// (`GROUND`'s fold finer) where anything is held finer than its cells, else its
/// cells, which then fold nothing.
pub(super) fn runs_zoom(c: PyramidChunk, finest: Zoom) -> Zoom {
    match c.cell_zoom() {
        cell if cell < finest => cell + GROUND.fold(),
        cell => cell,
    }
}

/// The zoom a chunk's runs are written at, and whether any object is held
/// over its column or the columns beside it, all heights: the runs are the
/// whole column's, and its apron reaches one cell into each neighbour.
pub fn runs_of(store: &Store, c: PyramidChunk) -> (Zoom, bool) {
    let (zoom, [x, y]) = c.column();
    let mut finest: Option<Zoom> = None;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if let Some(b) = store.bounds((zoom, [x + dx, y + dy]), c.cell_zoom()).value().flatten() {
                finest = Some(finest.map_or(b.finest, |f| f.max(b.finest)));
            }
        }
    }
    (runs_zoom(c, finest.map_or(GROUND.cell(), |z| z.max(GROUND.cell()))), finest.is_some())
}

/// The runs' cells, X and Y, a chunk's objects are folded over: its own and
/// one of apron, which a fold reads (`tiers_draw`).
pub fn folded_cells(c: PyramidChunk, runs: Zoom) -> ([i64; 2], [i64; 2]) {
    let (zoom, at) = c.node();
    let [x, y, _] = first_descendant_of(at, u32::from(runs - zoom));
    let n = i64::from(count(runs, zoom));
    ([x - 1, y - 1], [x + n + 1, y + n + 1])
}

/// The world's units a run's cell is tall: a leaf cell's, or past the
/// leaves the run's cell.
pub(super) fn z_unit(runs: Zoom) -> f64 {
    units(runs.max(GROUND.cell()))
}

/// Whether `c` draws what is held at `held` as it is, not folded into a
/// coarser cell: only there is it struck, hidden or cut by a preview.
pub fn draws_as_held(c: PyramidChunk, held: Zoom) -> bool {
    c.cell_zoom() >= held
}

/// Run columns a row, the apron's among them.
pub(super) fn run_span(c: PyramidChunk, runs: Zoom) -> u32 {
    CHUNK_CELLS * count(runs, c.cell_zoom()) + 2 * APRON
}
