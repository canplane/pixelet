// pxl-kernel/layer/terrain/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! the terrain's record and each kind's seed (the catalog's,
//! spec/spec/cell/decor.json), so a cell grows the same wherever it is read.

use std::collections::TryReserveError;

use pxl_render::mesh::paint::terrain::sample::TileSource;
use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::GroundCell;
use pxl_spec::cell::cube_types::Ground;
use pxl_spec::cell::decor::{DecorKind, DecorMeta, Orientation};
use pxl_spec::cell::Kind;
use pxl_spec::procedural::{scatter, Candidate};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{count, index, units, Zoom};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_spec::space::layer::tile::LEAF_ZOOM;
use xpute_core::math::rng::{u32_to_unit, SplitMix64};

use crate::layer::instance::{Instance, Meta, Source};

// Variety between instances without any reading as the wrong size.
pub const SCALE_JITTER: f64 = 0.18;

// A fixed radius (O(1) reads, no flood fill), enough to outvote a one-cell
// stairstep jog in a straight run; it does not resolve curves.
const ALIGN_NEIGHBOR_RADIUS: i64 = 3;

/// A plurality vote of transport cells along each axis: PI/2 if horizontal
/// wins, else 0 (a tie or isolated cell has no real answer). `flip` adds PI,
/// as two-way traffic faces both ways.
pub fn transport_aligned_rotation(cells: &[GroundCell], cell_x: u32, cell_y: u32, flip: bool) -> f64 {
    let span = 0..TILE_CELLS as i64;
    let (x, y) = (cell_x as i64, cell_y as i64);
    let way = |cell: GroundCell| Ground::of(cell).is_some_and(|g| g.kind() == Kind::Transport);
    let at = |x: i64, y: i64| span.contains(&x) && span.contains(&y) && way(cells[GROUND.cell_index(x as u32, y as u32) as usize]);
    let (mut horizontal_votes, mut vertical_votes) = (0, 0);
    for d in 1..=ALIGN_NEIGHBOR_RADIUS {
        horizontal_votes += at(x - d, y) as u32 + at(x + d, y) as u32;
        vertical_votes += at(x, y - d) as u32 + at(x, y + d) as u32;
    }
    let axis_rotation = if horizontal_votes > vertical_votes { core::f64::consts::PI / 2.0 } else { 0.0 };
    if flip {
        axis_rotation + core::f64::consts::PI
    } else {
        axis_rotation
    }
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

pub struct Draw {
    pub gx: i64,
    pub gy: i64,
    pub variant: u32,
    pub rotation: f64,
    pub scale: f64,
}

/// What `candidate` draws in the tile's cell `tile_at`: none off the kind's
/// ground or under a line.
pub fn draw(kind: &DecorKind, candidate: Candidate, cells: &[GroundCell], lines: &Decks, tile_at: [u32; 2]) -> Option<Draw> {
    let [cell_x, cell_y] = tile_at;
    if !kind.allows(cells[GROUND.cell_index(cell_x, cell_y) as usize]) {
        return None;
    }
    let mut rng = Rng::new(candidate.seed);
    let variant = ((rng.unit() * kind.variant_count() as f64).floor() as u32).min(kind.variant_count() - 1);
    let rotation = if kind.orientation == Orientation::along_transport {
        let flip = rng.unit() < 0.5;
        transport_aligned_rotation(cells, cell_x, cell_y, flip)
    } else {
        rng.unit() * core::f64::consts::PI * 2.0
    };
    let scale = 1.0 + (rng.unit() * 2.0 - 1.0) * SCALE_JITTER;
    if lines.over_point(None, [f64::from(cell_x) + 0.5, f64::from(cell_y) + 0.5], 1.0).next().is_some() {
        return None;
    }
    let [gx, gy] = candidate.cell;
    Some(Draw { gx, gy, variant, rotation, scale })
}

/// What the ground grows of `kind` in `region`, read off `source`'s cells at
/// `zoom` from its cell `first`, `side` of them a side, whose first lies at
/// `origin`: each candidate stands on its own cell's look and decks and at the
/// height the chunk draws there (`TileSource::cell_at`), so what is grown is
/// what is drawn. A tree coarser than a leaf's stands on the grass the view
/// sees, not on leaf cells it cannot. `candidates` is the caller's room.
#[allow(clippy::too_many_arguments)]
pub fn grow<E: From<TryReserveError>>(
    kind: &DecorKind,
    source: &TileSource,
    zoom: Zoom,
    origin: [i64; 2],
    first: [u32; 2],
    side: u32,
    region: Aabb,
    candidates: &mut Vec<Candidate>,
    mut sink: impl FnMut(Instance) -> Result<(), E>,
) -> Result<(), E> {
    let tile = &source.tile;
    let (own, atom) = (GROUND.cell_zoom(zoom), GROUND.cell_zoom(LEAF_ZOOM));
    let per = i64::from(count(atom, own));
    let min = [index(atom, origin[0]), index(atom, origin[1])];
    let span = i64::from(side) * per;
    scatter(kind.seed, kind.density, zoom, [min, [min[0] + span, min[1] + span]], candidates)?;
    for &candidate in candidates.iter() {
        let (dx, dy) = ((candidate.cell[0] - min[0]).div_euclid(per) as u32, (candidate.cell[1] - min[1]).div_euclid(per) as u32);
        let at = [first[0] + dx, first[1] + dy];
        let Some(Draw { gx, gy, variant, rotation, scale }) = draw(kind, candidate, tile.cells, tile.decks, at) else {
            continue;
        };
        let (_, ele) = source.cell_at(at.map(i64::from));
        let stands = [(gx as f64 + 0.5) * units(atom), (gy as f64 + 0.5) * units(atom), f64::from(ele)];
        // In the region holding the cell it stands on.
        if !((0..2).all(|k| (region.min[k]..region.max[k]).contains(&stands[k])) && region.min[2] < stands[2] && stands[2] <= region.max[2]) {
            continue;
        }
        sink(Instance {
            id: kind.value(),
            at: stands,
            source: Source::Ground([gx, gy, i64::from(ele)]),
            meta: Meta::Decor(DecorMeta { variant, rotation, scale }),
        })?;
    }
    Ok(())
}
