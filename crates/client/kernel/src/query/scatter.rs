// pxl-kernel/query/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! each decor's seed (the catalog's, spec/spec/block/decor.json) and the
//! ground's face as the world answers it, so a cell grows the same wherever
//! it is read. It reads no tile: where it may stand is the world's
//! (`World::grows`), which way it faces and how high it stands its face's
//! (`World::surfaces`).

use std::collections::TryReserveError;

use pxl_spec::block::cube::Ground;
use pxl_spec::block::decor::{Decor, Orientation};
use pxl_spec::procedural::{scatter, scatter_at, Candidate};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{count, index, units, Zoom};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_kit::math::rng::{u32_to_unit, SplitMix64};

use super::surface::Surfaces;
use super::world::World;
use crate::layer::instance::{Instance, Meta, Source};

// A fixed radius (O(1) reads, no flood fill), enough to outvote a one-cell
// stairstep jog in a straight run; it does not resolve curves.
const ALIGN_NEIGHBOR_RADIUS: i64 = 3;

/// A plurality vote of transport cells along each axis about `at`: a quarter
/// turn if horizontal wins, else none (a tie or isolated cell has no real
/// answer). `flip` adds a half, as two-way traffic faces both ways.
fn transport_aligned_turn(face: &Surfaces, [x, y]: [i64; 2], flip: bool) -> u32 {
    let way = |x: i64, y: i64| u32::from(face.look([x, y]).and_then(Ground::of).is_some_and(Ground::transport));
    let (mut horizontal_votes, mut vertical_votes) = (0, 0);
    for d in 1..=ALIGN_NEIGHBOR_RADIUS {
        horizontal_votes += way(x - d, y) + way(x + d, y);
        vertical_votes += way(x, y - d) + way(x, y + d);
    }
    u32::from(horizontal_votes > vertical_votes) + 2 * u32::from(flip)
}

struct Rng(SplitMix64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(SplitMix64::new(seed))
    }

    fn unit(&mut self) -> f64 {
        u32_to_unit(self.0.next_u32())
    }
}

/// The variant a candidate draws, its seed's first draw.
fn variant_of(decor: &Decor, rng: &mut Rng) -> u32 {
    ((rng.unit() * decor.variant_count() as f64).floor() as u32).min(decor.variant_count() - 1)
}

/// The square a candidate claims, which holds its box however it turns.
fn side_of(decor: &Decor, c: &Candidate) -> i64 {
    let [x, y, _] = decor.variants[variant_of(decor, &mut Rng::new(c.seed)) as usize].size;
    i64::from(x.max(y))
}

/// What the ground grows of `decor` in `region`, on the node `node` of grid
/// cells at `cell`, whose first leaf cell's corner lies at `origin`: each
/// candidate faces as its cell's face says and stands at the height the
/// chunk draws there, so what is grown is what is drawn. A tree coarser than
/// a leaf's stands on the grass the view sees, not on leaf cells it cannot. A
/// box is counted in leaf cells, so the ground grows a kind of `scale_steps`
/// 0 alone. `candidates` is the caller's room.
#[allow(clippy::too_many_arguments)]
pub fn grow<E: From<TryReserveError>>(
    world: &World,
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
    scatter(
        decor.seed,
        decor.density,
        zoom,
        [min, [min[0] + span, min[1] + span]],
        i64::from(decor.widest()),
        |c| side_of(decor, c),
        candidates,
    )?;
    let (x0, y0) = GROUND.read_at(node, cell);
    let unit = units(atom);
    let grown = world.surfaces(node, cell, ALIGN_NEIGHBOR_RADIUS as u32, |face| {
        for &candidate in candidates.iter() {
            let at = [0, 1].map(|k| i64::from([x0, y0][k]) + (candidate.cell[k] - min[k]).div_euclid(per));
            let Some(i) = stand(decor, face, candidate, at) else { continue };
            let Source::Ground([gx, gy, z]) = i.source else { continue };
            // In the region holding the cell it stands on.
            let at = [(gx as f64 + 0.5) * unit, (gy as f64 + 0.5) * unit, z as f64];
            if (0..2).all(|k| (region.min[k]..region.max[k]).contains(&at[k])) && region.min[2] < at[2] && at[2] <= region.max[2] {
                sink(i)?;
            }
        }
        Ok(())
    });
    grown.value().flatten().unwrap_or(Ok(()))
}

/// What the ground grows of `decor` in the leaf `cell`: the leaf scatter,
/// cell by cell, which a region grows the same (pxl-spec procedural.test.rs).
pub fn grown_at(world: &World, decor: &Decor, cell: [i64; 2]) -> Option<Instance> {
    let candidate = scatter_at(decor.seed, decor.density, cell, i64::from(decor.widest()), |c| side_of(decor, c))?;
    let node = (GROUND.cell_zoom(LEAF_ZOOM), cell);
    let grid = GROUND.grid_zoom(LEAF_ZOOM);
    let (x0, y0) = GROUND.read_at(node, grid);
    // Only the resident leaf can say.
    world
        .surfaces(node, grid, ALIGN_NEIGHBOR_RADIUS as u32, |face| stand(decor, face, candidate, [x0, y0].map(i64::from)))
        .exact()
        .flatten()
        .flatten()
}

/// A candidate on the face's cell `at`, at the height the chunk draws there;
/// none where the cell has no ground.
fn stand(decor: &Decor, face: &Surfaces, candidate: Candidate, at: [i64; 2]) -> Option<Instance> {
    let top = face.at(at)?.top;
    let mut rng = Rng::new(candidate.seed);
    let variant = variant_of(decor, &mut rng);
    let turn = match decor.orientation {
        Orientation::along_transport => {
            let flip = rng.unit() < 0.5;
            transport_aligned_turn(face, at, flip)
        }
        Orientation::random => (rng.unit() * 4.0) as u32,
    };
    let unit = units(GROUND.cell_zoom(LEAF_ZOOM));
    let [gx, gy] = candidate.cell;
    let (stands, meta) = decor.stand([gx as f64 * unit, gy as f64 * unit, top], variant, turn, 1.0);
    Some(Instance {
        id: decor.value(),
        at: stands,
        source: Source::Ground([gx, gy, top as i64]),
        meta: Meta::Decor(meta),
    })
}
