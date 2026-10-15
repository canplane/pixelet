// pxl-kernel/query/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! each decor's seed (the catalog's, spec/spec/block/decor.json) and the
//! ground's face as the world answers it, so a cell grows the same wherever
//! it is read. It reads no tile: where it may stand is the world's
//! (`World::grows`), which way it faces and how high it stands its face's
//! (`World::surfaces`).

use pxl_spec::space::layer::lattice::Cells;
use std::collections::TryReserveError;

use pxl_spec::block::cube::Ground;
use pxl_spec::block::decor::Decor;
use pxl_spec::procedural::{pose, scatter, scatter_at, Candidate, ALIGN_NEIGHBOR_RADIUS};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{count, index, units, Zoom};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::surface::Surfaces;
use super::world::World;
use crate::layer::instance::{Instance, Meta, Source};

/// Cells about a node its decor reads: the faces a turn is voted from, and
/// the widest box's footing past the cell it stands in.
pub fn ring(decors: &[Decor]) -> u32 {
    decors.iter().map(|d| d.row.widest()).fold(ALIGN_NEIGHBOR_RADIUS as u32, u32::max)
}

/// What the ground grows of `decor` in `region`, on the node `node` of grid
/// cells at `cell` read into `face` (`World::surfaces`, `ring` about it),
/// whose first leaf cell's corner lies at `origin`: each candidate faces as
/// its cell's face says, stands at the height the chunk draws there, and
/// grows where its footing does (`World::grows`), so what is grown is what is
/// drawn. A tree coarser than
/// a leaf's stands on the grass the view sees, not on leaf cells it cannot. A
/// box is counted in leaf cells, so the ground grows a kind of `scale_steps`
/// 0 alone. `placed` is what placed decor claims about `region`
/// (`World::placed_claims`), read once for every candidate in it;
/// `candidates` is the caller's room.
#[allow(clippy::too_many_arguments)]
pub fn grow<E: From<TryReserveError>>(
    world: &World,
    face: &mut Surfaces,
    decor: &Decor,
    node: Node<2>,
    cell: Zoom,
    origin: [i64; 2],
    region: Aabb,
    placed: &[Aabb],
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
    let (x0, y0) = GROUND.read_at(node, cell);
    let unit = units(atom);
    for &candidate in candidates.iter() {
        let at = [0, 1].map(|k| i64::from([x0, y0][k]) + (i64::from(candidate.cell[k]) - min[k]).div_euclid(per));
        let Some(i) = stand(decor, face, candidate, at) else { continue };
        let Source::Ground([gx, gy, z]) = i.source else { continue };
        // In the region holding the cell it stands on.
        let at = [(gx as f64 + 0.5) * unit, (gy as f64 + 0.5) * unit, z as f64];
        if (0..2).all(|k| (region.min[k]..region.max[k]).contains(&at[k])) && region.min[2] < at[2] && at[2] <= region.max[2] && world.grows(face, decor, &i, placed) {
            sink(i)?;
        }
    }
    Ok(())
}

/// What the ground stands of `decor` in the leaf `cell` on `face`, the
/// leaf's ground read about it (`World::grown_in`): the leaf scatter, cell
/// by cell, which a region grows the same (pxl-spec procedural.test.rs).
pub fn grown_at(face: &Surfaces, decor: &Decor, cell: [i64; 2]) -> Option<Instance> {
    let candidate = scatter_at(decor.row, cell.map(|v| v as i32))?;
    let (x0, y0) = GROUND.read_at((GROUND.cell_zoom(LEAF_ZOOM), cell), GROUND.grid_zoom(LEAF_ZOOM));
    stand(decor, face, candidate, [x0, y0].map(i64::from))
}

/// A candidate on the face's cell `at`, at the height the chunk draws there;
/// none where the cell has no ground.
fn stand(decor: &Decor, face: &Surfaces, candidate: Candidate, at: [i64; 2]) -> Option<Instance> {
    let top = face.at(at)?.top;
    let way = |[x, y]: [i32; 2]| face.look([at[0] + i64::from(x), at[1] + i64::from(y)]).and_then(Ground::of).is_some_and(Ground::transport);
    let (variant, turn) = pose(decor.row, &candidate, way);
    let unit = units(GROUND.cell_zoom(LEAF_ZOOM));
    let [gx, gy] = candidate.cell.map(i64::from);
    let (stands, meta) = decor.stand([gx as f64 * unit, gy as f64 * unit, top], variant, turn, 1.0);
    Some(Instance {
        id: decor.value(),
        at: stands,
        source: Source::Ground([gx, gy, top as i64]),
        meta: Meta::Decor(meta),
    })
}
