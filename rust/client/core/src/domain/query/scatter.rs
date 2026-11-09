// pxl-core/domain/query/scatter.rs

//! How the ground generates decor on its own. Generated decor is never
//! stored: it is a pure function of each decor's seed
//! (spec/spec/asset/decor.json) and of the world as its one query answers
//! it, so a cell generates the same decor wherever it is read. Where a decor
//! stands, which way it faces and whether it generates at all depend on the
//! world unplaced, the ground alone: its column's surface and the cells
//! around it (`World::column_surface`). Only an edit in its box, asked of
//! the world as edited, takes it away (pxl-content `generation::placement`).

use pxl_content::generation::{self, DecorQuery, Prop};
use pxl_content::scatter::scatter_at;
use pxl_engine::support::ColumnSurface;
use pxl_engine::world::PlacedCursor;
use pxl_spec::asset::cube::sentinel;
use pxl_spec::asset::decor::{self, Decor};
use pxl_spec::space::coord::q3::FRAC_W;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;

use super::node::At;
use super::world::World;
use crate::domain::layer::instance::{Instance, Source};
use crate::domain::layer::object::store;
use crate::domain::layer::Answer;

/// The world as decor's generation asks it on the CPU (pxl-engine
/// `DecorQuery`): the world's one query, both unplaced (`ground`, the ground
/// alone) and as edited, and what is placed (`lanes`).
pub(super) struct DecorWorld<'a, 'w> {
    world: &'a World<'w>,
    ground: World<'w>,
    lanes: store::Table<'a>,
}

impl<'a, 'w> DecorWorld<'a, 'w> {
    pub(super) fn new(world: &'a World<'w>) -> DecorWorld<'a, 'w> {
        DecorWorld {
            world,
            ground: world.ground_only(),
            lanes: world.object.columns(),
        }
    }
}

/// The world's one query on the CPU. A column's surface comes with its line
/// (`World::column_surface`), and each cell around the column is asked off
/// that line (`World::asset_about`), both of the world unplaced. A cell's
/// edit is asked of the world as edited (`World::asset`).
impl DecorQuery for DecorWorld<'_, '_> {
    type ColumnSurface = ColumnSurface<At>;

    fn rows(&self) -> &[decor::Row] {
        &decor::ROWS
    }

    /// The column's surface and its top, with false when the world lacked a
    /// tile on the way. Decor generates only on ground held exactly as its
    /// tiles say.
    fn column_surface(
        &mut self,
        cell_zoom: Zoom,
        g: [i32; 2],
    ) -> (ColumnSurface<At>, (i32, f32), bool) {
        let (answer, f) = self.ground.column_surface(cell_zoom, g);
        match answer {
            Answer::Exact(Some(top)) => (f, top, true),
            _ => (f, (0, 0.0), false),
        }
    }

    /// Always the leaves' cell zoom. The CPU generates decor at the leaves
    /// alone (`generated_at`), where its world answers each cell off its own
    /// tiles.
    fn finest(&mut self, _: Zoom, _: [i32; 2]) -> Zoom {
        GROUND.leaf_cell_zoom()
    }

    /// The asset of the cell `c` at `zoom`, void where the world holds
    /// nothing there. With `edited` the world as edited answers, so an edit
    /// shows where one is held, as far as its tiles are held.
    fn asset(&mut self, f: &ColumnSurface<At>, zoom: Zoom, c: [i32; 3], edited: bool) -> u32 {
        let answer = if edited {
            self.world.asset((zoom, c))
        } else {
            self.ground.asset_about(&f.line, zoom, c)
        };
        answer.val().unwrap_or(u32::from(sentinel::void))
    }

    fn next_placed(&self, p: &mut PlacedCursor) -> ([i32; 3], u32, bool) {
        p.next(&self.lanes)
    }
}

/// What the ground generates of `decor` in the leaf `cell`, reading the
/// leaf's ground around it (`World::generated_in`). This is the leaf
/// scatter, cell by cell, which a whole region generates the same way
/// (pxl-spec procedural.test.rs). With `boxed`, it also says whether the
/// decor generates there, its box asked against the same column surface
/// (pxl-content `generation::placement`); without `boxed` that is true.
pub(super) fn generated_at(
    world: &mut DecorWorld,
    decor: &Decor,
    cell: [i32; 2],
    boxed: bool,
) -> Option<(Instance, bool)> {
    let candidate = scatter_at(decor.row, cell)?;
    let (prop, has_footing, generates) =
        generation::placement(world, decor.row, GROUND.leaf_cell_zoom(), &candidate, boxed);
    has_footing.then(|| (instance(decor, &prop), generates))
}

/// The instance of decor standing as `prop`. Its box's min corner is at a leaf
/// cell's corner, and its base sinks under that corner as the GPU draws it
/// (`Prop::sink`).
pub(super) fn instance(decor: &Decor, prop: &Prop) -> Instance {
    let [x, y, z] = prop.at;
    let base = f64::from(z) - f64::from(prop.sink) / f64::from(1u32 << FRAC_W);
    let (at, meta) = decor.stand(
        [f64::from(x), f64::from(y), base],
        prop.variant,
        prop.turn,
        1.0,
    );
    Instance {
        id: decor.val(),
        at,
        src: Source::Ground([x, y, z]),
        meta,
    }
}
