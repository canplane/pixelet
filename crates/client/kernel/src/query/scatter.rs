// pxl-kernel/query/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! each decor's seed (the catalog's, spec/spec/block/decor.json) and the
//! ground's face as the world answers it, so a cell grows the same wherever
//! it is read. It reads no tile: where it may stand is the world's
//! (`World::grows`), which way it faces and how high it stands its face's
//! (`World::surfaces`).

use std::collections::TryReserveError;

use pxl_portable::placed;
use pxl_portable::scatter::{self as portable, Grounds, Grown, ALIGN_NEIGHBOR_RADIUS, PLACED_BLOCK};
use pxl_spec::block::cube::sentinel;
use pxl_spec::block::decor::Decor;
use pxl_spec::block::Block;
use pxl_spec::procedural::{scatter, scatter_at, Candidate};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{count, index, Zoom};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::surface::Surfaces;
use super::world::World;
use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::store;

/// Cells about a node its decor reads: the faces a turn is voted from, and
/// the widest box's footing past the cell it stands in.
pub fn ring(decors: &[Decor]) -> u32 {
    decors.iter().map(|d| d.row.widest()).fold(ALIGN_NEIGHBOR_RADIUS as u32, u32::max)
}

/// The world as decor's growth asks it on the CPU (pxl-portable
/// `Grounds`): the ground's cells off `face`, read once about what is
/// asked, the world's one query, and what is placed.
pub struct Asked<'a, 'w, 'f> {
    world: &'a World<'w>,
    face: &'a Surfaces<'f>,
    lanes: store::Lanes<'a>,
}

impl<'a, 'w, 'f> Asked<'a, 'w, 'f> {
    pub fn new(world: &'a World<'w>, face: &'a Surfaces<'f>) -> Asked<'a, 'w, 'f> {
        Asked {
            world,
            face,
            lanes: world.objects.lanes(),
        }
    }
}

impl placed::Lanes for Asked<'_, '_, '_> {
    fn heap(&self, slot: u32) -> u32 {
        self.lanes.heap(slot)
    }
    fn data(&self, slot: u32) -> u32 {
        self.lanes.data(slot)
    }
}

impl Grounds for Asked<'_, '_, '_> {
    /// Off `face`, whose cells are at `cell`; none past what it read.
    fn face(&mut self, cell: Zoom, g: [i32; 2]) -> (u32, i32, bool) {
        debug_assert_eq!(cell, self.face.cell());
        match self.face.ground(g.map(i64::from)) {
            Some((look, top)) => (u32::from(look), top, true),
            None => (0, 0, false),
        }
    }

    /// Void where the world holds nothing there.
    fn block(&mut self, zoom: Zoom, c: [i32; 3]) -> u32 {
        match self.world.block((zoom, c.map(i64::from))).value().flatten() {
            Some(Block::Ground(g, _)) => u32::from(g),
            Some(Block::Placed(v)) => PLACED_BLOCK | v,
            None => u32::from(sentinel::void),
        }
    }
}

/// What the ground grows of `decor` in `region`, on the node `node` of grid
/// cells at `cell` read into `face` (`World::surfaces`, `ring` about it),
/// whose first leaf cell's corner lies at `origin`: each candidate stands
/// and faces as its cells say, and grows where its footing does
/// (pxl-portable `stand`, `grows`), so what is grown is what is drawn. A
/// tree coarser than a leaf's stands on the grass the view sees, not on leaf
/// cells it cannot. `candidates` is the caller's room.
#[allow(clippy::too_many_arguments)]
pub fn grow<E: From<TryReserveError>>(
    world: &World,
    face: &Surfaces,
    decor: &Decor,
    node: Node<2>,
    cell: Zoom,
    origin: [i64; 2],
    region: Aabb,
    candidates: &mut Vec<Candidate>,
    mut sink: impl FnMut(Instance) -> Result<(), E>,
) -> Result<(), E> {
    let zoom = GROUND.read_zoom(cell);
    let (own, atom) = (GROUND.cell_zoom(zoom), GROUND.cell_zoom(LEAF_ZOOM));
    let per = i64::from(count(atom, own));
    let min = [index(atom, origin[0]), index(atom, origin[1])];
    let span = i64::from(GROUND.read_side(node.0, cell)) * per;
    let leaves = [min, [min[0] + span, min[1] + span]].map(|c| c.map(|v| v as i32));
    scatter(decor.row, zoom, leaves, candidates)?;
    let mut asked = Asked::new(world, face);
    for candidate in candidates.iter() {
        let Some(g) = portable::stand(&mut asked, decor.row, own, candidate) else { continue };
        // In the region holding the cell it stands on.
        let at = [f64::from(g.at[0]) + 0.5, f64::from(g.at[1]) + 0.5, f64::from(g.at[2])];
        if (0..2).all(|k| (region.min[k]..region.max[k]).contains(&at[k])) && region.min[2] < at[2] && at[2] <= region.max[2] && portable::grows(&mut asked, decor.row, own, &g) {
            sink(instance(decor, &g))?;
        }
    }
    Ok(())
}

/// What the ground stands of `decor` in the leaf `cell` on `face`, the
/// leaf's ground read about it (`World::grown_in`): the leaf scatter, cell
/// by cell, which a region grows the same (pxl-spec procedural.test.rs).
pub fn grown_at(asked: &mut Asked, decor: &Decor, cell: [i64; 2]) -> Option<Instance> {
    let candidate = scatter_at(decor.row, cell.map(|v| v as i32))?;
    portable::stand(asked, decor.row, GROUND.cell(), &candidate).map(|g| instance(decor, &g))
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
