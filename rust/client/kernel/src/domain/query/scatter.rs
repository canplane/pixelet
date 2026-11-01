// pxl-kernel/domain/query/scatter.rs

//! How the ground generates decor on its own: never stored, a pure function
//! of each decor's seed (the catalog's, spec/spec/asset/decor.json) and the
//! world as its one query answers it, so a cell generates the same wherever
//! it is read. Where it stands, which way it faces and whether it generates are
//! the world unplaced, the ground's alone, its column's surface
//! and the cells about it (`World::column_surface`); only an edit in its box, asked
//! of the world as edited, takes it away (pxl-spec-core `generation::placement`).

use pxl_spec::asset::cube::sentinel;
use pxl_spec::asset::decor::Decor;
use pxl_spec::procedural::scatter_at;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec_core::generation::{self, DecorQuery, GeneratedDecor};
use pxl_spec_core::support::ColumnSurface;
use pxl_spec_core::world::PlacedCursor;
use pxl_spec_core::FRAC_W;

use super::node::At;
use super::world::World;
use crate::domain::layer::instance::{Instance, Source};
use crate::domain::layer::object::store;
use crate::domain::layer::Answer;
use pxl_spec_core::decor;

/// The world as decor's generation asks it on the CPU (pxl-spec-core
/// `DecorQuery`): the world's one query and its ray, unplaced (`ground`,
/// the ground's alone) and as edited, and what is placed.
pub(super) struct DecorWorld<'a, 'w> {
    world: &'a World<'w>,
    ground: World<'w>,
    lanes: store::Columns<'a>,
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

/// The world's one query on the CPU, each ask answered with what its tiles
/// lacked: a column's surface with its line (`World::column_surface`) and each cell
/// about it asked off that line (`World::asset_about`), of the world
/// unplaced; a cell's edit asked of the world as edited (`World::asset`).
impl DecorQuery for DecorWorld<'_, '_> {
    type ColumnSurface = ColumnSurface<At>;

    fn rows(&self) -> &[decor::Decor] {
        &decor::DECORS
    }

    /// None where the world lacked a tile on the way: decor generates on what
    /// is held as its tiles say.
    fn column_surface(&mut self, cell: Zoom, g: [i32; 2]) -> (ColumnSurface<At>, (i32, f32), bool) {
        let (answer, f) = self.ground.column_surface(cell, g);
        match answer {
            Answer::Exact(Some(top)) => (f, top, true),
            _ => (f, (0, 0.0), false),
        }
    }

    /// The leaves', which the CPU generates at alone (`generated_at`), its
    /// world answering each cell off its own tiles.
    fn finest(&mut self, _: Zoom, _: [i32; 2]) -> Zoom {
        GROUND.cell()
    }

    /// Void where the world holds nothing there; as edited, an edit where it
    /// holds one, as far as it is held.
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

/// What the ground generates of `decor` in the leaf `cell`, the leaf's ground
/// read about it (`World::generated_in`): the leaf scatter, cell by cell, which
/// a region generates the same (pxl-spec procedural.test.rs); and, where
/// `boxed`, whether it generates there, its box asked about the same column surface
/// (pxl-spec-core `generation::placement`), true where not.
pub(super) fn generated_at(
    asked: &mut DecorWorld,
    decor: &Decor,
    cell: [i32; 2],
    boxed: bool,
) -> Option<(Instance, bool)> {
    let candidate = scatter_at(decor.row, cell)?;
    let (g, has_footing, generates) =
        generation::placement(asked, decor.row, GROUND.cell(), &candidate, boxed);
    has_footing.then(|| (instance(decor, &g), generates))
}

/// Decor standing as `g`, as an instance: its box's min corner a leaf
/// cell's, a unit, its base sunk under it as the GPU draws it (`GeneratedDecor::sink`).
pub(super) fn instance(decor: &Decor, g: &GeneratedDecor) -> Instance {
    let [x, y, z] = g.at;
    let base = f64::from(z) - f64::from(g.sink) / f64::from(1u32 << FRAC_W);
    let (at, meta) = decor.stand([f64::from(x), f64::from(y), base], g.variant, g.turn, 1.0);
    Instance {
        id: decor.val(),
        at,
        src: Source::Ground([x, y, z]),
        meta,
    }
}
