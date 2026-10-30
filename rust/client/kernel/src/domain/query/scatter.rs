// pxl-kernel/domain/query/scatter.rs

//! How the ground generates decor on its own: never stored, a pure function
//! of each decor's seed (the catalog's, spec/spec/asset/decor.json) and the
//! world as its one query answers it, so a cell generates the same wherever
//! it is read. Where it stands, which way it faces and whether it generates are
//! the world unplaced, the ground's alone, its column's footing
//! and the cells about it (`World::footing`); only an edit in its box, asked
//! of the world as edited, takes it away (pxl-spec-core `scatter::generated`).

use pxl_spec::asset::cube::sentinel;
use pxl_spec::asset::decor::Decor;
use pxl_spec::procedural::scatter_at;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec_core::scatter::{self as core_scatter, DecorQuery, Generated};
use pxl_spec_core::support::Footing;
use pxl_spec_core::world::Placed;
use pxl_spec_core::FRAC_W;

use super::node::At;
use super::world::World;
use crate::domain::layer::instance::{Instance, Source};
use crate::domain::layer::object::store;
use crate::domain::layer::Answer;
use pxl_spec_core::answer;
use pxl_spec_core::decor;

/// The world as decor's generation asks it on the CPU (pxl-spec-core
/// `DecorQuery`): the world's one query and its ray, unplaced (`unplaced`,
/// the ground's alone) and as edited, and what is placed.
pub(super) struct Asked<'a, 'w> {
    world: &'a World<'w>,
    unplaced: World<'w>,
    lanes: store::Lanes<'a>,
}

impl<'a, 'w> Asked<'a, 'w> {
    pub(super) fn new(world: &'a World<'w>) -> Asked<'a, 'w> {
        Asked {
            world,
            unplaced: world.unplaced(),
            lanes: world.object.lanes(),
        }
    }
}

/// The world's one query on the CPU, each ask answered with what its tiles
/// lacked: a column's footing with its line (`World::footing`) and each cell
/// about it asked off that line (`World::asset_about`), of the world
/// unplaced; a cell's edit asked of the world as edited (`World::asset`).
impl DecorQuery for Asked<'_, '_> {
    type Footing = Footing<At>;

    fn rows(&self) -> &[decor::Decor] {
        &decor::DECORS
    }

    /// None where the world lacked a tile on the way: decor generates on what
    /// is held as its tiles say.
    fn footing(&mut self, cell: Zoom, g: [i32; 2]) -> (Footing<At>, (i32, f32), bool) {
        let (answer, f) = self.unplaced.footing(cell, g);
        match answer {
            Answer::Exact(Some(top)) => (f, top, true),
            _ => (f, (0, 0.0), false),
        }
    }

    /// Void where the world holds nothing there.
    fn asset(&mut self, f: &Footing<At>, zoom: Zoom, c: [i32; 3]) -> u32 {
        self.unplaced
            .asset_about(&f.line, zoom, c)
            .val()
            .unwrap_or(u32::from(sentinel::void))
    }

    /// An edit where the world holds one there, as far as it is held.
    fn is_edited(&mut self, zoom: Zoom, c: [i32; 3]) -> bool {
        self.world
            .asset((zoom, c))
            .val()
            .is_some_and(answer::is_placed)
    }

    fn next_placed(&self, p: &mut Placed) -> ([i32; 3], u32, bool) {
        p.next(&self.lanes)
    }
}

/// What the ground generates of `decor` in the leaf `cell`, the leaf's ground
/// read about it (`World::generated_in`): the leaf scatter, cell by cell, which
/// a region generates the same (pxl-spec procedural.test.rs); and, where
/// `boxed`, whether it generates there, its box asked about the same footing
/// (pxl-spec-core `scatter::generated`), true where not.
pub(super) fn generated_at(
    asked: &mut Asked,
    decor: &Decor,
    cell: [i32; 2],
    boxed: bool,
) -> Option<(Instance, bool)> {
    let candidate = scatter_at(decor.row, cell)?;
    let (g, has_footing, generates) =
        core_scatter::generated(asked, decor.row, GROUND.cell(), &candidate, boxed);
    has_footing.then(|| (instance(decor, &g), generates))
}

/// Decor standing as `g`, as an instance: its box's min corner a leaf
/// cell's, a unit, its base sunk under it as the GPU draws it (`Generated::sink`).
pub fn instance(decor: &Decor, g: &Generated) -> Instance {
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
