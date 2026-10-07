// pxl-kernel/layer/terrain/scatter.rs

//! How the ground grows decor on its own: never stored, a pure function of
//! the terrain's record and each kind's seed (the catalog's,
//! spec/spec/cell/decor.json), so a cell grows the same wherever it is read.

use pxl_spec::bake::deck::Decks;
use pxl_spec::bake::format::GroundCell;
use pxl_spec::cell::decor::{DecorKind, Orientation};
use pxl_spec::cell::cube_types::Ground;
use pxl_spec::cell::Kind;
use pxl_spec::procedural::Candidate;
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};
use xpute_core::math::rng::{u32_to_unit, SplitMix64};

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
