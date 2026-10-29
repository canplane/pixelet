// pxl-kernel/domain/query/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! each decor's seed (the catalog's, spec/spec/asset/decor.json) and the
//! world as its one query answers it, so a cell grows the same wherever it
//! is read. Where it stands, which way it faces and whether it grows are
//! the world unplaced, the ground's alone, its column's footing
//! and the cells about it (`World::footing`); only an edit in its box, asked
//! of the world as edited, takes it away (pxl-spec-core `scatter::grown`).

use pxl_spec::asset::cube::sentinel;
use pxl_spec::asset::decor::Decor;
use pxl_spec::procedural::scatter_at;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec_core::scatter::{self as core_scatter, Grounds, Grown};
use pxl_spec_core::support::Footing;
use pxl_spec_core::world::Placed;
use pxl_spec_core::FRAC_W;

use super::node::At;
use super::world::World;
use crate::domain::layer::instance::{Instance, Source};
use crate::domain::layer::object::store;

/// The world as decor's growth asks it on the CPU (pxl-spec-core
/// `Grounds`): the world's one query and its ray, unplaced (`unplaced`,
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
impl Grounds for Asked<'_, '_> {
    type Footing = Footing<At>;

    fn rows(&self) -> &[pxl_spec_core::decor::Decor] {
        &pxl_spec_core::decor::DECORS
    }

    /// None where the world lacked a tile on the way: decor grows on what
    /// is held as its tiles say.
    fn footing(&mut self, cell: Zoom, g: [i32; 2]) -> (Footing<At>, (i32, f32), bool) {
        let (answer, f) = self.unplaced.footing(cell, g);
        match answer {
            crate::domain::layer::Answer::Exact(Some(top)) => (f, top, true),
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
            .is_some_and(pxl_spec_core::answer::is_placed)
    }

    fn next_placed(&self, p: &mut Placed) -> ([i32; 3], u32, bool) {
        p.next(&self.lanes)
    }
}

/// What the ground stands of `decor` in the leaf `cell`, the leaf's ground
/// read about it (`World::grown_in`): the leaf scatter, cell by cell, which
/// a region grows the same (pxl-spec procedural.test.rs); and, where
/// `boxed`, whether it grows there, its box asked about the same footing
/// (pxl-spec-core `scatter::grown`), true where not.
pub(super) fn grown_at(
    asked: &mut Asked,
    decor: &Decor,
    cell: [i32; 2],
    boxed: bool,
) -> Option<(Instance, bool)> {
    let candidate = scatter_at(decor.row, cell)?;
    let (g, stood, grows) = core_scatter::grown(asked, decor.row, GROUND.cell(), &candidate, boxed);
    stood.then(|| (instance(decor, &g), grows))
}

/// Decor standing as `g`, as an instance: its box's min corner a leaf
/// cell's, a unit, its base sunk under it as the GPU draws it (`Grown::sink`).
pub fn instance(decor: &Decor, g: &Grown) -> Instance {
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
