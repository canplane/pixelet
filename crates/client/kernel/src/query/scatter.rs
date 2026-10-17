// pxl-kernel/query/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! each decor's seed (the catalog's, spec/spec/block/decor.json) and the
//! ground's face as the world answers it, so a cell grows the same wherever
//! it is read. It reads no tile: where it may stand is the world's
//! (`World::grows`), which way it faces and how high it stands its face's
//! (`World::surfaces`).

use pxl_portable::overwrite;
use pxl_portable::scatter::{self as portable, Grounds, Grown, PLACED_BLOCK};
use pxl_spec::block::cube::sentinel;
use pxl_spec::block::decor::Decor;
use pxl_spec::block::Block;
use pxl_spec::procedural::scatter_at;
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;

use super::surface::Surfaces;
use super::world::World;
use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::store;

/// The world as decor's growth asks it on the CPU (pxl-portable
/// `Grounds`): the ground's cells off `face`, each read as it is asked, the
/// world's one query, and what is placed.
pub(super) struct Asked<'a, 'w> {
    world: &'a World<'w>,
    face: &'a Surfaces,
    lanes: store::Lanes<'a>,
}

impl<'a, 'w> Asked<'a, 'w> {
    pub(super) fn new(world: &'a World<'w>, face: &'a Surfaces) -> Asked<'a, 'w> {
        Asked {
            world,
            face,
            lanes: world.objects.lanes(),
        }
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

    /// Off `face`, whose cells are at `cell`; none past what it read.
    fn face(&mut self, cell: Zoom, g: [i32; 2]) -> (u32, i32, bool) {
        debug_assert_eq!(cell, self.face.cell());
        match self.face.ground(g.map(i64::from)) {
            Some((look, top)) => (u32::from(look), top, true),
            None => (0, 0, false),
        }
    }

    fn start(&mut self, zoom: Zoom, c: [i32; 3]) -> Self::At {
        self.world.start((zoom, c.map(i64::from)))
    }

    /// Void where the world holds nothing there.
    fn block(&mut self, at: &Self::At, from: Zoom, zoom: Zoom, c: [i32; 3]) -> u32 {
        let mut at = *at;
        for z in from + 1..=zoom {
            let s = zoom - z;
            at = self.world.child(&at, (0..3).map(|i| (((c[i] >> s) & 1) as usize) << i).sum());
        }
        match self.world.drawn(&mut at).value().flatten() {
            Some(Block::Ground(g)) => u32::from(g),
            Some(Block::Placed(v)) => PLACED_BLOCK | v,
            None => u32::from(sentinel::void),
        }
    }
}

/// What the ground stands of `decor` in the leaf `cell` on `face`, the
/// leaf's ground read about it (`World::grown_in`): the leaf scatter, cell
/// by cell, which a region grows the same (pxl-spec procedural.test.rs).
pub(super) fn grown_at(asked: &mut Asked, decor: &Decor, cell: [i64; 2]) -> Option<Instance> {
    let candidate = scatter_at(decor.row, cell.map(|v| v as i32))?;
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
        source: Source::Ground([x, y, z].map(i64::from)),
        meta: Meta::Decor(meta),
    }
}

/// An instance the ground grows as portable's evaluation takes it back.
pub fn grown_of(i: &Instance) -> Option<Grown> {
    let (Source::Ground([x, y, z]), Meta::Decor(m)) = (i.source, i.meta) else { return None };
    Some(Grown {
        variant: m.variant,
        turn: m.turn,
        at: [x, y, z].map(|v| v as i32),
    })
}
