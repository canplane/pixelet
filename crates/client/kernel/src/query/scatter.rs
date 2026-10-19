// pxl-kernel/query/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! each decor's seed (the catalog's, spec/spec/block/decor.json) and the
//! world as its one query answers it, so a cell grows the same wherever it
//! is read and whichever layer holds what it stands on. It reads no tile:
//! where it stands is its column's footing, the world's ray down it
//! (`World::footing`), and which way it faces and whether it grows the
//! world's blocks about it (pxl-portable `scatter::stand`, `grows`).

use pxl_portable::overwrite;
use pxl_portable::scatter::{self as portable, Grounds, Grown};
use pxl_spec::block::cube::sentinel;
use pxl_spec::block::decor::Decor;
use pxl_spec::procedural::scatter_at;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;

use super::node::At;
use super::ray::Walking;
use super::world::World;
use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::store;
use crate::layer::residency::Lines;
use pxl_portable::cast;

/// The world as decor's growth asks it on the CPU (pxl-portable
/// `Grounds`): the world's one query and its ray, and what is placed.
pub(super) struct Asked<'a, 'w> {
    world: &'a World<'w>,
    lanes: store::Lanes<'a>,
    /// What the footing's walk holds of its line, none before one.
    line: Option<cast::Line<[At; cast::DEPTH], At>>,
    /// The ground's lines the footing's walk read, each block's stepped
    /// from them.
    lines: Lines,
}

impl<'a, 'w> Asked<'a, 'w> {
    pub(super) fn new(world: &'a World<'w>) -> Asked<'a, 'w> {
        Asked {
            world,
            lanes: world.objects.lanes(),
            line: None,
            lines: Lines::default(),
        }
    }
}

/// What is over a footing's walk, from the world's roots: nothing, so it is
/// never asked (pxl-portable `cast::Rooted`, its `top` 0).
struct Unasked;

impl cast::Over<At> for Unasked {
    fn over(&self, zoom: Zoom) -> At {
        xpute_kit::bug!(EINVAL, zoom)
    }
}

impl overwrite::Lanes for Asked<'_, '_> {
    fn heap(&self, slot: u32) -> u32 {
        self.lanes.heap(slot)
    }
    fn data(&self, slot: u32) -> u32 {
        self.lanes.data(slot)
    }
}

impl Grounds for Asked<'_, '_> {
    type At = super::node::At;

    fn rows(&self) -> &[pxl_portable::decor::Decor] {
        &pxl_portable::decor::DECORS
    }

    fn footing(&mut self, cell: Zoom, g: [i32; 2]) -> (u32, i32, bool) {
        match self.world.footing(cell, g) {
            Some(f) => {
                (self.line, self.lines) = (Some(f.line), f.lines);
                (f.block, f.top, true)
            }
            None => (0, 0, false),
        }
    }

    fn held(&mut self, up: Zoom, node: (Zoom, [i32; 3])) -> (Zoom, Self::At) {
        match self.line.map(|l| l.held::<Walking>(up, node, &Unasked)) {
            Some((zoom, at, true)) => (zoom, at),
            _ => (0, self.root(node)),
        }
    }

    fn root(&mut self, (zoom, c): (Zoom, [i32; 3])) -> Self::At {
        self.world.start((0, c.map(|v| v >> zoom)))
    }

    /// Void where the world holds nothing there.
    fn block(&mut self, at: &Self::At, from: Zoom, zoom: Zoom, c: [i32; 3]) -> u32 {
        let mut at = *at;
        for z in from + 1..=zoom {
            let s = zoom - z;
            at = self.world.child(&at, (0..3).map(|i| (((c[i] >> s) & 1) as usize) << i).sum());
        }
        self.world.drawn(&mut at, &mut self.lines).value().unwrap_or(u32::from(sentinel::void))
    }
}

/// What the ground stands of `decor` in the leaf `cell` on `face`, the
/// leaf's ground read about it (`World::grown_in`): the leaf scatter, cell
/// by cell, which a region grows the same (pxl-spec procedural.test.rs).
pub(super) fn grown_at(asked: &mut Asked, decor: &Decor, cell: [i32; 2]) -> Option<Instance> {
    let candidate = scatter_at(decor.row, cell)?;
    let (g, here) = portable::stand(asked, decor.row, GROUND.cell(), &candidate);
    here.then(|| instance(decor, &g))
}

/// Decor standing as `g`, as an instance: its box's min corner a leaf
/// cell's, a unit.
pub fn instance(decor: &Decor, g: &Grown) -> Instance {
    let [x, y, z] = g.at;
    let (at, meta) = decor.stand([x, y, z].map(f64::from), g.variant, g.turn, 1.0);
    Instance {
        id: decor.value(),
        at,
        source: Source::Ground([x, y, z]),
        meta: Meta::Decor(meta),
    }
}

/// An instance the ground grows as portable's evaluation takes it back.
pub fn grown_of(i: &Instance) -> Option<Grown> {
    let (Source::Ground([x, y, z]), Meta::Decor(m)) = (i.source, i.meta) else { return None };
    Some(Grown {
        variant: m.variant,
        turn: m.turn,
        at: [x, y, z],
    })
}
